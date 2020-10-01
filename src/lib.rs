/*
 * This library helps convert an XML String into a serde_json::Value which can be
 * used to generate JSON
 */

#[cfg(test)]
#[macro_use]
extern crate serde_json;

use log::*;
use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value};

#[derive(Debug)]
pub struct Error {
}


fn read<'a>(mut reader: &mut Reader<&'a [u8]>) -> Value {
    let mut buf = Vec::new();
    let mut values = Vec::new();
    let mut node = Map::new();

    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let mut attrs = Map::new();

                if let Ok(name) = String::from_utf8(e.name().to_vec()) {
                    let mut child = read(&mut reader);
                    debug!("{} children: {:?}", name, child);
                    let mut has_attrs = false;

                    e.attributes().map(|a| {
                        if let Ok(attr) = a {
                            let key = String::from_utf8(attr.key.to_vec());
                            let value = String::from_utf8(attr.value.to_vec());

                            // Only bother adding the attribute if both key and value are valid utf8
                            if key.is_ok() && value.is_ok() {
                                let key = format!("@{}", key.unwrap());
                                let value = Value::String(value.unwrap());

                                // If the child is already an object, that's where the insert
                                // should happen
                                if child.is_object() {
                                    child.as_object_mut().unwrap().insert(key, value);
                                }
                                else {
                                    has_attrs = true;
                                    attrs.insert(key, value);
                                }
                            }
                        }
                    }).collect::<Vec<_>>();

                    /* 
                     * nodes with attributes need to be handled special
                     */
                    if attrs.len() > 0 {
                        if child.is_string() {
                            attrs.insert("#text".to_string(), child);
                        }

                        if let Ok(attrs) = serde_json::to_value(attrs) {
                            node.insert(name, attrs);
                        }
                    }
                    else {
                        if node.contains_key(&name) {
                            debug!("Node contains `{}` already, need to convert to array", name);
                            let (_, mut existing) = node.remove_entry(&name).unwrap();
                            let mut entries: Vec<Value> = vec![];

                            if existing.is_array() {
                                let existing = existing.as_array_mut().unwrap();
                                while existing.len() > 0 {
                                    entries.push(existing.remove(0));
                                }
                            }
                            else {
                                entries.push(existing);
                            }
                            entries.push(child);

                            node.insert(name, Value::Array(entries));
                        }
                        else {
                            node.insert(name, child);
                        }
                    }
                }

                if let Ok(node_value) = serde_json::to_value(&node) {
                    debug!("pushing node_value: {:?}", node_value);
                    values.push(node_value);
                }
            },
            Ok(Event::Text(e)) => {
                if let Ok(decoded) = e.unescape_and_decode(&reader) {
                    values.push(Value::String(decoded));
                }
            },
            Ok(Event::CData(e)) => {
                if let Ok(decoded) = e.unescape_and_decode(&reader) {
                    node.insert("#cdata".to_string(), Value::String(decoded));
                }
            },
            Ok(Event::End(ref _e)) => break,
            Ok(Event::Eof) => break,
            _ => (),
        }
    }

    debug!("values to return: {:?}", values);
    if node.len() > 0 {
        // If we had collected some text along the way, that needs to be inserted
        // so we don't lose it
        let mut index = 0;
        let mut has_text = false;
        for value in values.iter() {
            if value.is_string() {
                has_text = true;
                break;
            }
            index += 1;
        }

        if has_text {
            node.insert("#text".to_string(), values.remove(index));
        }
        debug!("returning node instead: {:?}", node);
        return serde_json::to_value(&node).expect("Failed to #to_value() a node!");
    }

    match values.len() {
        0 => Value::Null,
        1 => values.pop().unwrap(),
        _ => {
            Value::Array(values)
        }
    }
}

/**
 * to_json() will take an input string and attempt to convert it into a form
 * of JSON
 */
pub fn to_json(xml: &str) -> Result<Value, Error> {
    let mut reader = Reader::from_str(xml);
    reader.expand_empty_elements(true);
    reader.trim_text(true);

    Ok(read(&mut reader))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_eq(left: Value, right: Result<Value, Error>) {
        assert!(right.is_ok());
        assert_eq!(left, right.unwrap());
    }

    #[test]
    fn single_node() {
        json_eq(
            json!({"e" : null}),
            to_json("<e></e>")
        );
    }

    #[test]
    fn node_with_text() {
        json_eq(
            json!({"e" : "foo"}),
            to_json("<e>foo</e>")
        );
    }

    #[test]
    fn node_with_attr() {
        json_eq(
            json!({"e" : {"@name":"value"}}),
            to_json("<e name=\"value\"></e>")
        );
    }

    #[test]
    fn node_with_attr_and_text() {
        json_eq(
            json!({"e": {"@name":"value", "#text" : "text"}}),
            to_json(r#"<e name="value">text</e>"#)
        );
    }

    #[test]
    fn node_with_children() {
        json_eq(
            json!(
                {
                "e":{
                    "a":"text1",
                    "b":"text2"
                }
                }),
            to_json(r#"<e> <a>text1</a> <b>text2</b> </e>"#)
        );
    }

    #[test]
    fn node_with_multiple_identical_children() {
        json_eq(
            json!({
                "e":{"a":[
                    "text",
                    "text"
                    ]}
                }),
            to_json(r#"<e><a>text</a><a>text</a></e>"#)
        );
    }

    #[test]
    fn node_with_N_identical_children() {
        json_eq(
            json!({
                "e":{"a":[
                    "text1",
                    "text2",
                    "text3"
                    ]}
                }),
            to_json(r#"<e><a>text1</a><a>text2</a><a>text3</a></e>"#)
        );
    }


    #[test]
    fn node_with_text_and_child() {
        json_eq(json!(
            {
            "e":{
                "#text":"lol",
                "a":"text"
            }
            }),
            to_json(r#"<e> lol <a>text</a></e>"#)
        );
    }

    #[test]
    fn node_with_just_text() {
        json_eq(json!(
            {
            "a":"hello"
            }),
            to_json(r#"<a>hello</a>"#)
        );
    }

    #[test]
    fn node_with_attrs_and_text() {
        json_eq(json!(
            {
                "a":{
                    "@x":"y",
                    "#text":"hello"
                }
            }),
            to_json(r#"<a x="y">hello</a>"#)
        );
    }

    #[test]
    fn nested_nodes_with_attrs() {
        json_eq(json!(
            {
                "a":{
                    "@id":"a",
                    "b":{
                    "@id":"b",
                    "#text":"hey!"
                    }
                }
            }),
            to_json(r#"<a id="a"><b id="b">hey!</b></a>"#)
        );
    }

    #[test]
    fn node_with_nested_test() {
        /*
        todo!("this syntax makes no sense to me");
        json_eq(json!(
            {
                "a":"x<c/>y"
            }),
            to_json(r#"<a>x<c/>y</a>"#)
        );
        */
    }

    #[test]
    fn node_with_empty_attrs() {
        json_eq(json!(
            {
            "x":{"@u":""}
            }),
            to_json(r#"<x u=""/>"#)
        );
    }

    #[test]
    fn some_basic_html() {
        json_eq(json!(
            {
            "html":{
                "head":{
                "title":"Xml/Json",
                "meta":{
                    "@name":"x",
                    "@content":"y"
                }
                },
                "body":null
            }
            }),
            to_json(r#"<html><head><title>Xml/Json</title><meta name="x" content="y"/></head><body/></html>"#)
        );
    }


    #[test]
    fn more_complex_html() {
        json_eq(json!(
            {
                "ol":{
                    "@class":"xoxo",
                    "li":[
                    {
                        "#text":"Subject 1",
                        "ol":{"li":[
                            "subpoint a",
                            "subpoint b"
                        ]}
                    },
                    {
                        "span":"Subject 2",
                        "ol":{
                        "@compact":"compact",
                        "li":[
                            "subpoint c",
                            "subpoint d"
                        ]
                        }
                    }
                    ]
                }
            }),
            to_json(r#"<ol class="xoxo"><li>Subject 1     <ol><li>subpoint a</li><li>subpoint b</li></ol></li><li><span>Subject 2</span><ol compact="compact"><li>subpoint c</li><li>subpoint d</li></ol></li></ol>"#)
            );
    }

    #[test]
    fn node_with_cdata() {
        json_eq(json!(
            {
            "e":{"#cdata":" .. some data .. "}
            }),
            to_json(r#"<e><![CDATA[ .. some data .. ]]></e>"#)
        );
    }

    #[test]
    fn node_with_cdata_and_siblings() {
        json_eq(json!(
            {
            "e":{
                "a":null,
                "#cdata":" .. some data .. ",
                "b":null
            }
            }),
            to_json(r#"<e><a/><![CDATA[ .. some data .. ]]><b/></e>"#)
        );
    }

    #[test]
    fn node_with_cdata_inside_text() {
        /*
         * TODO
        json_eq(json!(
            {
            "e":"\n  some text\n  <![CDATA[ .. some data .. ]]>\n  more text\n"
            }),
            to_json(r#"<e>  some text  <![CDATA[ .. some data .. ]]>  more text</e>"#)
        );
        */
    }

    #[test]
    fn node_with_child_cdata_and_text() {
        json_eq(json!(
            {
            "e":{
                "#text":"some text",
                "#cdata":" .. some data .. ",
                "a":null
            }
            }),
            to_json(r#"<e>  some text  <![CDATA[ .. some data .. ]]><a/></e>"#)
        );
    }

    #[test]
    fn node_with_duplicate_cdata() {
        /*
         * TODO: unsure about this approach to handling cdata
        json_eq(json!(
            {
            "e":"<![CDATA[ .. some data .. ]]><![CDATA[ .. more data .. ]]>"
            }
            ),
            to_json(r#"<e><![CDATA[ .. some data .. ]]><![CDATA[ .. more data .. ]]></e>"#)
        );
        */
    }
}
