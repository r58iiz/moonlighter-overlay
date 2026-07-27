use crate::matchers::Rect;
use crate::overlay::draw_list::OverlayMode;
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Debug, Clone)]
pub enum AppCommand {
    SetOverlayMode(OverlayMode),
    ToggleOverlayMode(OverlayMode),
    ToggleDebugMode,
    ToggleNgPlusMode,
    RequestRedetect,
    DumpTemplates,
    Exit,

    // Marking mode actions
    SaveMarkedSlots,
    ClearMarkedSlots,
    AddMarkedSlot(Rect),
    RemoveLastMarkedSlot,
    UpdateCurrentDrag(Option<(u32, u32, u32, u32)>),

    // Search query actions
    UpdateSearchQuery(String),
}

pub struct CommandBus {
    sender: Sender<AppCommand>,
}

impl CommandBus {
    pub fn new(sender: Sender<AppCommand>) -> Self {
        Self { sender }
    }

    pub fn send(&self, cmd: AppCommand) {
        let _ = self.sender.send(cmd);
    }

    pub fn clone_sender(&self) -> Sender<AppCommand> {
        self.sender.clone()
    }
}

pub fn create_command_bus() -> (Sender<AppCommand>, Receiver<AppCommand>) {
    channel()
}
