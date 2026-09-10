//! Extra VT events use vt100's existing parser callbacks. There is one screen
//! model/parser in the daemon, and no terminal-text heuristics for agent state.
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use std::collections::{BTreeMap, VecDeque};
#[derive(Clone, Debug, Default)]
pub struct Notice {
    pub title: String,
    pub body: String,
}
#[derive(Default)]
pub struct Events {
    pub replies: VecDeque<String>,
    pub notices: VecDeque<Notice>,
    chunks: BTreeMap<String, Notice>,
}
fn text(bytes: &[u8], limit: usize) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}
impl Events {
    fn reply(&mut self, reply: String) {
        if self.replies.len() < 64 {
            self.replies.push_back(reply);
        }
    }
    fn notice(&mut self, title: String, body: String) {
        if title.is_empty() && body.is_empty() {
            return;
        }
        if self.notices.len() == 16 {
            self.notices.pop_front();
        }
        self.notices.push_back(Notice { title, body });
    }
    fn kitty(&mut self, params: &[&[u8]]) {
        if params.len() < 3 {
            return;
        }
        let metadata = String::from_utf8_lossy(params[1]);
        let fields = metadata
            .split(':')
            .filter_map(|s| s.split_once('='))
            .collect::<BTreeMap<_, _>>();
        let id = fields.get("i").copied().unwrap_or("0");
        if id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return;
        }
        let part = fields.get("p").copied().unwrap_or("title");
        if part == "?" {
            self.reply(format!("\x1b]99;i={id}:p=?;p=title,body:o=always\x1b\\"));
            return;
        }
        if !matches!(part, "title" | "body") {
            return;
        }
        let joined = params[2..].join(&b';');
        let payload = if fields.get("e") == Some(&"1") {
            let Ok(data) = STANDARD
                .decode(&joined)
                .or_else(|_| STANDARD_NO_PAD.decode(&joined))
            else {
                return;
            };
            data
        } else {
            joined
        };
        if !self.chunks.contains_key(id) && self.chunks.len() >= 16 {
            self.chunks.pop_first();
        }
        let chunk = self.chunks.entry(id.into()).or_default();
        let target = if part == "title" {
            &mut chunk.title
        } else {
            &mut chunk.body
        };
        let remaining = 1024usize.saturating_sub(target.chars().count());
        target.push_str(&text(&payload, remaining));
        if fields.get("d") != Some(&"0")
            && let Some(notice) = self.chunks.remove(id)
        {
            self.notice(notice.title, notice.body);
        }
    }
}
impl vt100::Callbacks for Events {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        if i2.is_some() {
            return;
        }
        let arg = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (i1, c, arg) {
            (None, 'c', 0) => self.reply("\x1b[?6c".into()),
            (Some(b'>'), 'c', 0) => self.reply("\x1b[>0;100;1c".into()),
            (None, 'n', 5) => self.reply("\x1b[0n".into()),
            (None, 'n', 6) => {
                let (row, col) = screen.cursor_position();
                self.reply(format!("\x1b[{};{}R", row + 1, col + 1));
            }
            (None, 't', 18) => {
                let (rows, cols) = screen.size();
                self.reply(format!("\x1b[8;{rows};{cols}t"));
            }
            // Do not advertise keyboard modes that the daemon parser does not track.
            _ => {}
        }
    }
    fn paste_from_clipboard(&mut self, _: &mut vt100::Screen, selector: &[u8]) {
        self.reply(format!("\x1b]52;{};\x1b\\", text(selector, 16)));
    }
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        match params.first().copied().unwrap_or_default() {
            b"9" if params.len() > 1 => {
                // OSC 9;4 is ConEmu progress, not a desktop notification.
                if params.len() > 2 && params[1] == b"4" {
                    return;
                }
                self.notice("Terminal".into(), text(&params[1..].join(&b';'), 1024));
            }
            b"777" if params.len() >= 3 && params[1] == b"notify" => {
                self.notice(text(params[2], 256), text(&params[3..].join(&b';'), 1024));
            }
            b"99" => self.kitty(params),
            b"10" | b"11" | b"12" if params.get(1) == Some(&b"?".as_slice()) => {
                let channel = text(params[0], 2);
                let c = if channel == "11" {
                    0x1818u16
                } else {
                    0xd8d8u16
                };
                self.reply(format!("\x1b]{channel};rgb:{c:04x}/{c:04x}/{c:04x}\x1b\\"));
            }
            b"4" if params.len() == 3 && params[2] == b"?" => {
                if let Ok(index) = String::from_utf8_lossy(params[1]).parse::<u8>() {
                    self.reply(format!("\x1b]4;{index};rgb:d8d8/d8d8/d8d8\x1b\\"));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replies_use_the_same_screen_as_reconnect_snapshots() {
        let mut parser = vt100::Parser::new_with_callbacks(24, 80, 100, Events::default());
        parser.process(b"\x1b[4;7H\x1b[6n\x1b[5n\x1b[c\x1b]11;?\x07");
        let replies = &parser.callbacks().replies;
        assert_eq!(replies[0], "\x1b[4;7R");
        assert_eq!(replies[1], "\x1b[0n");
        assert_eq!(replies[2], "\x1b[?6c");
        assert!(replies[3].contains("1818"));
        parser.screen_mut().set_size(40, 100);
        parser.process(b"\x1b[18t");
        assert_eq!(parser.callbacks().replies.back().unwrap(), "\x1b[8;40;100t");
    }
    #[test]
    fn parses_chunked_notifications_without_confusing_progress_or_agent_state() {
        let mut p = vt100::Parser::new_with_callbacks(24, 80, 100, Events::default());
        for bytes in [
            b"\x1b]9;hello\x07".as_slice(),
            b"\x1b]777;notify;Title;Body\x1b\\",
            b"\x1b]99;i=one:d=0;Kitty\x1b\\",
            b"\x1b]99;i=one:p=body;done\x1b\\",
            b"\x1b]9;4;1;50\x07",
        ] {
            for byte in bytes {
                p.process(&[*byte]);
            }
        }
        assert_eq!(p.callbacks().notices.len(), 3);
        let last = p.callbacks().notices.back().unwrap();
        assert_eq!((&*last.title, &*last.body), ("Kitty", "done"));
        let mut replay = vt100::Parser::new_with_callbacks(24, 80, 100, Events::default());
        replay.process(&p.screen().state_formatted());
        assert!(replay.callbacks().notices.is_empty());
        assert!(replay.callbacks().replies.is_empty());
    }
    #[test]
    fn notification_queues_and_payloads_are_bounded() {
        let mut p = vt100::Parser::new_with_callbacks(24, 80, 100, Events::default());
        for _ in 0..100 {
            p.process(format!("\x1b]9;{}\x07", "x".repeat(8000)).as_bytes());
        }
        assert!(p.callbacks().notices.len() <= 16);
        assert!(p.callbacks().notices.iter().all(|n| n.body.len() <= 1024));
    }
    #[test]
    fn snapshot_retains_wide_cells_colors_alt_screen_and_input_modes() {
        let mut p = vt100::Parser::new_with_callbacks(24, 80, 100, Events::default());
        p.process(
            "scrollback\r\n\x1b[?1049h\x1b[?2004h\x1b[?1000h\x1b[31m日本\x1b[4;5Hcursor".as_bytes(),
        );
        let mut replay = vt100::Parser::new(24, 80, 100);
        if p.screen().alternate_screen() {
            replay.process(b"\x1b[?1049h");
        }
        replay.process(&p.screen().state_formatted());
        assert_eq!(replay.screen().contents(), p.screen().contents());
        assert_eq!(
            replay.screen().cursor_position(),
            p.screen().cursor_position()
        );
        assert_eq!(
            replay.screen().alternate_screen(),
            p.screen().alternate_screen()
        );
        assert_eq!(
            replay.screen().input_mode_formatted(),
            p.screen().input_mode_formatted()
        );
        assert_eq!(
            replay.screen().cell(0, 0).unwrap().fgcolor(),
            p.screen().cell(0, 0).unwrap().fgcolor()
        );
    }
}
