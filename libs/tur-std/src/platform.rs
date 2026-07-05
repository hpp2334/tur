use tur_shared::Cursor;

pub trait CursorPlatform {
    fn set_cursor(&mut self, cursor: Cursor);
}

pub trait ClipboardPlatform {
    fn write_text(&self, text: &str);
    fn read_text(&self) -> Option<String>;
}

pub struct NoopCursorPlatform;
impl CursorPlatform for NoopCursorPlatform {
    fn set_cursor(&mut self, _cursor: Cursor) {}
}

pub struct NoopClipboardPlatform;
impl ClipboardPlatform for NoopClipboardPlatform {
    fn write_text(&self, _text: &str) {}
    fn read_text(&self) -> Option<String> {
        None
    }
}
