use tree_sitter::{Language, Node, Point, Tree, TreeCursor};

extern "C" {
    pub fn tree_sitter_json() -> Language;
}

pub struct JsonFile {
    pub text: String,
    pub tree: Tree,
}

trait CursorHelpers {
    fn goto_next_named_sibling(&mut self) -> bool;
    fn goto_first_named_child(&mut self) -> bool;
}

impl CursorHelpers for TreeCursor<'_> {
    fn goto_next_named_sibling(&mut self) -> bool {
        loop {
            if !self.goto_next_sibling() {
                return false;
            }
            if self.node().is_named() {
                return true;
            }
        }
    }

    fn goto_first_named_child(&mut self) -> bool {
        if !self.goto_first_child() {
            return false;
        }
        return self.goto_next_named_sibling();
    }
}

trait NodeHelpers {
    fn is_string_equal(&self, source: &str, text: &str) -> bool;
}

impl<'a> NodeHelpers for Node<'a> {
    fn is_string_equal(&self, source: &str, text: &str) -> bool {
        // A string node has `"`, string_content, `"`. So the string_content is at 1.
        if let Some(string_content) = self.child(1) {
            return string_content.utf8_text(source.as_bytes()) == Ok(text);
        } else {
            return false;
        }
    }
}

impl JsonFile {
    pub fn find_definition_for_key(&self, key: &[&str]) -> Option<Point> {
        let mut cursor = self.tree.walk();
        cursor.goto_first_child(); // document -> object
        cursor.goto_first_child(); // object -> open brace
        println!("{:?}", cursor.node().kind());
        if cursor.node().kind() != "{" {
            // Just bail out if we're not starting on an object
            return None;
        }
        self.find_definition_for_key_with_cursor(key, &mut cursor)
    }

    fn find_definition_for_key_with_cursor(
        &self,
        key: &[&str],
        cursor: &mut TreeCursor,
    ) -> Option<Point> {
        if key.is_empty() {
            return Some(cursor.node().start_position());
        }
        println!("calling");

        let head = key[0];
        if !cursor.goto_next_named_sibling() {
            return None;
        }
        // Loop through pairs
        loop {
            let node = cursor.node();
            if let Some(pair_key) = node.child_by_field_name("key") {
                println!("we got the pair key, {:?}, {:?}", pair_key, pair_key.utf8_text(self.text.as_bytes()));
                if pair_key.is_string_equal(&self.text, head) {
                    return self.find_definition_for_key_with_cursor(&key[1..], cursor);
                }
            }
            if !cursor.goto_next_named_sibling() {
                return None;
            }
        }
        // loop {
        //     println!("looping");
        //     let pair_key = cursor.node().child_by_field_name("key");
        //     println!("we got the pair key, {:?}", pair_key);
        //     if let Some(pair_key) = pair_key {
        //         if pair_key.is_string_equal(&self.text, head) {
        //             return Some(cursor.node().child_by_field_name("value")?.start_position());
        //         }
        //     }
        //     if !cursor.goto_next_sibling() {
        //         return None;
        //     }
        // }
    }
}
