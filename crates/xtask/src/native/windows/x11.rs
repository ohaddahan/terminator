use super::*;
use x11rb::{
    CURRENT_TIME,
    connection::Connection,
    protocol::{
        xproto::{self, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, MapState},
        xtest::ConnectionExt as _,
    },
    rust_connection::RustConnection,
};
pub struct Desktop {
    connection: RustConnection,
    root: u32,
    window: u32,
    output: PathBuf,
}
impl Desktop {
    pub fn preflight() -> Result<()> {
        ensure!(
            std::env::var_os("TERMINATOR_X11_TEST").is_some(),
            "Run the X11 fixture in an isolated Xvfb/Openbox display with TERMINATOR_X11_TEST=1"
        );
        Ok(())
    }
    fn atom(&self, name: &str) -> Result<u32> {
        Ok(self
            .connection
            .intern_atom(false, name.as_bytes())?
            .reply()?
            .atom)
    }
    pub fn new(pid: u32, output: &Path) -> Result<Self> {
        let (connection, screen) = x11rb::connect(None)?;
        let root = connection.setup().roots[screen].root;
        let mut desktop = Self {
            connection,
            root,
            window: 0,
            output: output.into(),
        };
        wait(|| {
            let atom = desktop.atom("_NET_CLIENT_LIST")?;
            let list = desktop
                .connection
                .get_property(false, root, atom, AtomEnum::WINDOW, 0, 256)?
                .reply()?;
            for window in list.value32().into_iter().flatten() {
                let value = desktop
                    .connection
                    .get_property(
                        false,
                        window,
                        desktop.atom("_NET_WM_PID")?,
                        AtomEnum::CARDINAL,
                        0,
                        1,
                    )?
                    .reply()?;
                if value.value32().and_then(|mut v| v.next()) == Some(pid) {
                    desktop.window = window;
                    return Ok(true);
                }
            }
            Ok(false)
        })?;
        Ok(desktop)
    }
    pub fn geometry(&self) -> Result<[f64; 4]> {
        let geometry = self.connection.get_geometry(self.window)?.reply()?;
        let position = self
            .connection
            .translate_coordinates(self.window, self.root, 0, 0)?
            .reply()?;
        Ok([
            position.dst_x as f64,
            position.dst_y as f64,
            geometry.width as f64,
            geometry.height as f64,
        ])
    }
    fn event(&self, kind: u8, detail: u8, x: f64, y: f64) -> Result<()> {
        self.connection
            .xtest_fake_input(kind, detail, CURRENT_TIME, self.root, x as i16, y as i16, 0)?
            .check()?;
        self.connection.flush()?;
        thread::sleep(Duration::from_millis(45));
        Ok(())
    }
    pub fn click(&mut self, x: f64, y: f64) -> Result<()> {
        self.event(xproto::MOTION_NOTIFY_EVENT, 0, x, y)?;
        self.event(xproto::BUTTON_PRESS_EVENT, 1, x, y)?;
        self.event(xproto::BUTTON_RELEASE_EVENT, 1, x, y)
    }
    pub fn drag(&mut self, start: (f64, f64), end: (f64, f64)) -> Result<()> {
        self.event(xproto::MOTION_NOTIFY_EVENT, 0, start.0, start.1)?;
        self.event(xproto::BUTTON_PRESS_EVENT, 1, start.0, start.1)?;
        for i in 1..=10 {
            self.event(
                xproto::MOTION_NOTIFY_EVENT,
                0,
                start.0 + (end.0 - start.0) * i as f64 / 10.0,
                start.1 + (end.1 - start.1) * i as f64 / 10.0,
            )?;
        }
        self.event(xproto::BUTTON_RELEASE_EVENT, 1, end.0, end.1)
    }
    pub fn minimize(&mut self) -> Result<()> {
        let r = self.geometry()?;
        self.click(r[0] + 52.0, r[1] + 20.0)
    }
    pub fn minimized(&self) -> Result<bool> {
        Ok(self
            .connection
            .get_window_attributes(self.window)?
            .reply()?
            .map_state
            != MapState::VIEWABLE)
    }
    pub fn restore(&self) -> Result<()> {
        self.connection.map_window(self.window)?.check()?;
        let event = ClientMessageEvent::new(
            32,
            self.window,
            self.atom("_NET_ACTIVE_WINDOW")?,
            [2, CURRENT_TIME, 0, 0, 0],
        );
        self.connection
            .send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            )?
            .check()?;
        self.connection.flush()?;
        Ok(())
    }
    pub fn close(&mut self) -> Result<()> {
        let r = self.geometry()?;
        self.click(r[0] + 20.0, r[1] + 20.0)
    }
    fn dialog(&self) -> Result<Option<u32>> {
        let list = self
            .connection
            .get_property(
                false,
                self.root,
                self.atom("_NET_CLIENT_LIST")?,
                AtomEnum::WINDOW,
                0,
                256,
            )?
            .reply()?;
        for window in list.value32().into_iter().flatten() {
            if window == self.window {
                continue;
            }
            let title = self
                .connection
                .get_property(
                    false,
                    window,
                    self.atom("_NET_WM_NAME")?,
                    AtomEnum::ANY,
                    0,
                    1024,
                )?
                .reply()?;
            let title = String::from_utf8_lossy(&title.value);
            if title == "Open file" || title == "Open project" {
                return Ok(Some(window));
            }
        }
        Ok(None)
    }
    fn capture_dialog(&self) -> Result<()> {
        let Some(window) = self.dialog()? else {
            return Ok(());
        };
        let geometry = self.connection.get_geometry(window)?.reply()?;
        let image = self
            .connection
            .get_image(
                xproto::ImageFormat::Z_PIXMAP,
                window,
                0,
                0,
                geometry.width,
                geometry.height,
                u32::MAX,
            )?
            .reply()?;
        let format = self
            .connection
            .setup()
            .pixmap_formats
            .iter()
            .find(|f| f.depth == image.depth)
            .context("Pixel format missing")?;
        ensure!(
            format.bits_per_pixel == 32,
            "Unsupported X11 capture pixel format"
        );
        let stride = (usize::from(geometry.width) * 4)
            .div_ceil(usize::from(format.scanline_pad) / 8)
            * (usize::from(format.scanline_pad) / 8);
        let pixels =
            image::RgbaImage::from_fn(geometry.width.into(), geometry.height.into(), |x, y| {
                let i = y as usize * stride + x as usize * 4;
                image::Rgba([image.data[i + 2], image.data[i + 1], image.data[i], 255])
            });
        pixels.save(self.output.join("native-picker.png"))?;
        Ok(())
    }
    fn keycode(&self, symbol: u32) -> Result<(u8, bool)> {
        let setup = self.connection.setup();
        let map = self
            .connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)?
            .reply()?;
        for (index, symbols) in map
            .keysyms
            .chunks(map.keysyms_per_keycode as usize)
            .enumerate()
        {
            for (shift, &code) in symbols.iter().take(2).enumerate() {
                if code == symbol {
                    return Ok((setup.min_keycode + index as u8, shift == 1));
                }
            }
        }
        anyhow::bail!("Key not in X11 keyboard map: {symbol}")
    }
    fn keys(&self, modifiers: &[u32], symbol: u32) -> Result<()> {
        let (code, shift) = self.keycode(symbol)?;
        let mut modifiers = modifiers.to_vec();
        if shift {
            modifiers.push(0xffe1);
        }
        for key in &modifiers {
            self.event(xproto::KEY_PRESS_EVENT, self.keycode(*key)?.0, 0.0, 0.0)?;
        }
        self.event(xproto::KEY_PRESS_EVENT, code, 0.0, 0.0)?;
        self.event(xproto::KEY_RELEASE_EVENT, code, 0.0, 0.0)?;
        for key in modifiers.iter().rev() {
            self.event(xproto::KEY_RELEASE_EVENT, self.keycode(*key)?.0, 0.0, 0.0)?;
        }
        Ok(())
    }
    pub fn open_file_shortcut(&self) -> Result<()> {
        self.keys(&[0xffe3, 0xffe1], b'o' as u32)
    }
    pub fn escape(&self) -> Result<()> {
        self.keys(&[], 0xff1b)
    }
    pub fn choose_path(&mut self, path: &str) -> Result<()> {
        if std::path::Path::new(path).is_dir() {
            let window = self.dialog()?.context("Native folder chooser missing")?;
            let position = self
                .connection
                .translate_coordinates(window, self.root, 0, 0)?
                .reply()?;
            let geometry = self.connection.get_geometry(window)?.reply()?;
            // GTK's Recent view has no ordinary filesystem model. Enter Browse
            // through Home before using the location entry to choose a folder.
            self.click(
                f64::from(position.dst_x) + 70.0,
                f64::from(position.dst_y) + 60.0,
            )?;
            thread::sleep(Duration::from_millis(250));
            self.keys(&[0xffe3], b'l' as u32)?;
            self.keys(&[0xffe3], b'a' as u32)?;
            for byte in format!("{}/", path.trim_end_matches('/')).bytes() {
                self.keys(&[], byte as u32)?;
            }
            thread::sleep(Duration::from_millis(200));
            self.keys(&[], 0xff0d)?;
            thread::sleep(Duration::from_millis(500));
            self.capture_dialog()?;
            self.click(
                f64::from(position.dst_x) + f64::from(geometry.width) - 48.0,
                f64::from(position.dst_y) + f64::from(geometry.height) - 22.0,
            )?;
            return Ok(());
        }
        self.keys(&[0xffe3], b'l' as u32)?;
        thread::sleep(Duration::from_millis(200));
        for byte in path.bytes() {
            self.keys(&[], byte as u32)?;
        }
        self.keys(&[], 0xff0d)?;
        thread::sleep(Duration::from_millis(500));
        self.keys(&[], 0xff0d)
    }
}
