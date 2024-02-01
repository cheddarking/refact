use std::sync::Arc;
use serde_json::{json, Value};
use tokio::sync::RwLock as ARwLock;
use crate::at_commands::structs::{AtCommand, AtCommandsContext};

use crate::call_validation::{ChatMessage, ChatPost};
use crate::global_context::GlobalContext;


pub async fn run_at_commands(
    global_context: Arc<ARwLock<GlobalContext>>,
    post: &mut ChatPost,
    top_n: usize,
    has_vecdb: &mut HasVecdbResults,
) {
    // TODO: don't operate on `post`, return a copy of the messages
    let context = AtCommandsContext::new(global_context.clone()).await;
    let mut query = post.messages.last().unwrap().content.clone(); // latest_msg_cont
    let valid_commands = crate::at_commands::utils::find_valid_at_commands_in_query(&mut query, &context).await;

    for cmd in valid_commands {
        match cmd.command.lock().await.execute(&query, &cmd.args, top_n, &context).await {
            Ok(msg) => {
                post.messages.push(msg.clone());
                has_vecdb.push_in_json(json!(msg));
            },
            Err(_) => {}
        }
    }
    let msg = ChatMessage{
        role: "user".to_string(),
        content: query,  // stream back to the user, without commands
    };
    post.messages.push(msg.clone());
    has_vecdb.push_in_json(json!(msg));
}

pub struct HasVecdbResults {
    pub was_sent: bool,
    pub in_json: Vec<Value>,
}

impl HasVecdbResults {
    pub fn new() -> Self {
        HasVecdbResults {
            was_sent: false,
            in_json: vec![],
        }
    }
}

impl HasVecdbResults {
    pub fn push_in_json(&mut self, value: Value) {
        self.in_json.push(value);
    }

    pub fn response_streaming(&mut self) -> Result<Vec<Value>, String> {
        if self.was_sent == true || self.in_json.is_empty() {
            return Ok(vec![]);
        }
        self.was_sent = true;
        Ok(self.in_json.clone())
    }
}
