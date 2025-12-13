use eu4txt::{DefaultEU4Txt, EU4Txt, from_node};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::io::Write;
use tempfile::NamedTempFile;

fn deserialize_from_str<T: DeserializeOwned>(data: &str) -> T {
    let mut file = NamedTempFile::new().expect("TempFile");
    write!(file, "{}", data).expect("Write");
    let path = file.path().to_str().unwrap();
    let tokens = DefaultEU4Txt::open_txt(path).expect("Tokenize");
    let ast = DefaultEU4Txt::parse(tokens).expect("Parse");
    from_node(&ast).expect("Deserialize")
}

#[derive(Debug, Deserialize, PartialEq)]
struct Simple {
    foo: i32,
    bar: String,
}

#[test]
fn test_simple_struct() {
    let data = r#"
        foo = 123
        bar = "hello"
    "#;
    let s: Simple = deserialize_from_str(data);
    assert_eq!(
        s,
        Simple {
            foo: 123,
            bar: "hello".to_string()
        }
    );
}

#[derive(Debug, Deserialize, PartialEq)]
struct BoolTest {
    is_true: bool,
    is_false: bool,
}

#[test]
fn test_bools() {
    let data = r#"
        is_true = yes
        is_false = no
    "#;
    let s: BoolTest = deserialize_from_str(data);
    assert_eq!(
        s,
        BoolTest {
            is_true: true,
            is_false: false
        }
    );
}

#[derive(Debug, Deserialize, PartialEq)]
struct ListTest {
    nums: Vec<i32>,
    names: Vec<String>,
}

#[test]
fn test_lists() {
    let data = r#"
        nums = { 1 2 3 }
        names = { "a" "b" c }
    "#;
    let s: ListTest = deserialize_from_str(data);
    assert_eq!(s.nums, vec![1, 2, 3]);
    assert_eq!(s.names, vec!["a", "b", "c"]);
}

#[derive(Debug, Deserialize, PartialEq)]
struct Nested {
    inner: Simple,
}

#[test]
fn test_nested() {
    let data = r#"
        inner = {
            foo = 999
            bar = "inner"
        }
    "#;
    let s: Nested = deserialize_from_str(data);
    assert_eq!(
        s.inner,
        Simple {
            foo: 999,
            bar: "inner".to_string()
        }
    );
}
