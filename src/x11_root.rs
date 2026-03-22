use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt as _, PropMode, Window};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub struct RootNameWriter {
    conn: RustConnection,
    root: Window,
    net_wm_name: Atom,
    utf8_string: Atom,
}

impl RootNameWriter {
    /// Open the active X session.
    pub fn connect() -> Result<Self, String> {
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|err| format!("Failed to connect to X11: {err}"))?;
        let root = conn.setup()
            .roots
            .get(screen_num)
            .ok_or_else(|| format!("Invalid X11 screen index: {screen_num}"))?
            .root;
        let net_wm_name = _intern_atom(&conn, b"_NET_WM_NAME")?;
        let utf8_string = _intern_atom(&conn, b"UTF8_STRING")?;

        Ok(Self {
            conn,
            root,
            net_wm_name,
            utf8_string,
        })
    }

    /// Update the root window name.
    pub fn set_status(&self, text: &str) -> Result<(), String> {
        self.conn.change_property8(
            PropMode::REPLACE,
            self.root,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            text.as_bytes(),
        )
        .map_err(|err| format!("Failed to queue WM_NAME update: {err}"))?;

        self.conn.change_property8(
            PropMode::REPLACE,
            self.root,
            self.net_wm_name,
            self.utf8_string,
            text.as_bytes(),
        )
        .map_err(|err| format!("Failed to queue _NET_WM_NAME update: {err}"))?;

        self.conn.flush()
            .map_err(|err| format!("Failed to flush X11 updates: {err}"))?;

        Ok(())
    }
}

/// Resolve one atom name.
fn _intern_atom(conn: &RustConnection, name: &[u8]) -> Result<Atom, String> {
    conn.intern_atom(false, name)
        .map_err(|err| format!("Failed to send intern_atom for {:?}: {err}", name))?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|err| format!("Failed to resolve atom {:?}: {err}", name))
}
