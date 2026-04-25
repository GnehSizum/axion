#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(String);

impl WindowId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn main() -> Self {
        Self::new("main")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for WindowId {
    fn default() -> Self {
        Self::main()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowConfig {
    pub id: WindowId,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub visible: bool,
}

impl WindowConfig {
    pub fn new(id: WindowId, title: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            id,
            title: title.into(),
            width,
            height,
            resizable: true,
            visible: true,
        }
    }

    pub fn main(title: impl Into<String>) -> Self {
        Self::new(WindowId::main(), title, 960, 720)
    }
}
