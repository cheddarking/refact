use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex as AMutex;
use tokio::sync::RwLock as ARwLock;

use crate::at_tools::at_tools::{at_tools_dict, AtTool};
use crate::call_validation::{ContextFile, ContextEnum};
use crate::global_context::GlobalContext;

use crate::at_commands::at_workspace::AtWorkspace;
use crate::at_commands::at_file::AtFile;
use crate::at_commands::at_ast_definition::AtAstDefinition;
use crate::at_commands::at_ast_reference::AtAstReference;
use crate::at_commands::at_ast_lookup_symbols::AtAstLookupSymbols;
use crate::at_commands::at_local_notes_to_self::AtLocalNotesToSelf;


pub struct AtCommandsContext {
    pub global_context: Arc<ARwLock<GlobalContext>>,
    pub at_commands: HashMap<String, Arc<AMutex<Box<dyn AtCommand + Send>>>>,
    pub at_tools: HashMap<String, Arc<AMutex<Box<dyn AtTool + Send>>>>,
    pub top_n: usize,
}

impl AtCommandsContext {
    pub async fn new(global_context: Arc<ARwLock<GlobalContext>>, top_n: usize) -> Self {
        AtCommandsContext {
            global_context,
            at_commands: at_commands_dict().await,
            at_tools: at_tools_dict().await,
            top_n,
        }
    }
}

#[async_trait]
pub trait AtCommand: Send + Sync {
    fn params(&self) -> &Vec<Arc<AMutex<dyn AtParam>>>;
    // returns (messages_for_postprocessing, text_on_clip)
    async fn execute(&self, ccx: &mut AtCommandsContext, query: &String, args: &Vec<String>) -> Result<(Vec<ContextEnum>, String), String>;
    fn depends_on(&self) -> Vec<String> { vec![] }   // "ast", "vecdb"
}

#[async_trait]
pub trait AtParam: Send + Sync {
    fn name(&self) -> &String;
    async fn is_value_valid(&self, value: &String, ccx: &AtCommandsContext) -> bool;
    async fn complete(&self, value: &String, ccx: &AtCommandsContext) -> Vec<String>;
    fn complete_if_valid(&self) -> bool {false}
}

pub struct AtCommandCall {
    pub command: Arc<AMutex<Box<dyn AtCommand + Send>>>,
    pub args: Vec<String>,
}

impl AtCommandCall {
    pub fn new(command: Arc<AMutex<Box<dyn AtCommand + Send>>>, args: Vec<String>) -> Self {
        AtCommandCall {
            command,
            args,
        }
    }
}

pub async fn at_commands_dict() -> HashMap<String, Arc<AMutex<Box<dyn AtCommand + Send>>>> {
    return HashMap::from([
        ("@workspace".to_string(), Arc::new(AMutex::new(Box::new(AtWorkspace::new()) as Box<dyn AtCommand + Send>))),
        ("@file".to_string(), Arc::new(AMutex::new(Box::new(AtFile::new()) as Box<dyn AtCommand + Send>))),
        ("@definition".to_string(), Arc::new(AMutex::new(Box::new(AtAstDefinition::new()) as Box<dyn AtCommand + Send>))),
        ("@references".to_string(), Arc::new(AMutex::new(Box::new(AtAstReference::new()) as Box<dyn AtCommand + Send>))),
        ("@symbols-at".to_string(), Arc::new(AMutex::new(Box::new(AtAstLookupSymbols::new()) as Box<dyn AtCommand + Send>))),
        ("@local-notes-to-self".to_string(), Arc::new(AMutex::new(Box::new(AtLocalNotesToSelf::new()) as Box<dyn AtCommand + Send>))),
    ]);
}

pub fn vec_context_file_to_context_tools(x: Vec<ContextFile>) -> Vec<ContextEnum> {
    x.into_iter().map(|i|ContextEnum::ContextFile(i)).collect::<Vec<ContextEnum>>()
}

pub fn filter_only_context_file_from_context_tool(tools: &Vec<ContextEnum>) -> Vec<ContextFile> {
    tools.iter()
        .filter_map(|x| {
            if let ContextEnum::ContextFile(data) = x { Some(data.clone()) } else { None }
        }).collect::<Vec<ContextFile>>()
}
