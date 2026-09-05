//! `DumpState` (the client's per-frame server payload) and its `default_*`
//! functions, split out of `src/client_json.rs` to respect the
//! file-structure line-count gate. Re-exported at `crate::client::DumpState`
//! (kept there since `tests-rs/test_zdep_json_types.rs` names it by that
//! path). See `src/client_json.rs` for the sibling types this depends on.

use psmux_json::{Error, FromJson, ToJson, Value};

use super::client_json::{BindingEntry, CustomizeOption, FloatJson, ServerMenuItem, WinStatus};
use crate::json_util::{def, def_fn, opt, req};
use crate::layout::{LayoutJson, RowRunsJson};
use crate::rendering::dim_predictions_enabled;
use crate::util::WinTree;

fn default_base_index() -> usize { 1 }
fn default_prediction_dimming() -> bool { dim_predictions_enabled() }
fn default_status_left_length() -> usize { 10 }
fn default_status_right_length() -> usize { 40 }
fn default_status_lines() -> usize { 1 }
fn default_status_visible() -> bool { true }
fn default_repeat_time() -> u64 { 500 }
fn default_bold_is_bright() -> bool { true }
fn default_paste_detection() -> bool { true }
fn default_mouse_selection() -> bool { true }
fn default_scroll_enter_copy_mode() -> bool { true }

pub(crate) struct DumpState {
    pub layout: LayoutJson,
    pub windows: Vec<WinStatus>,
    pub prefix: Option<String>,
    pub prefix2: Option<String>,
    pub tree: Vec<WinTree>,
    pub base_index: usize,
    pub prediction_dimming: bool,
    pub status_style: Option<String>,
    pub status_left: Option<String>,
    pub status_right: Option<String>,
    pub pane_border_style: Option<String>,
    pub pane_active_border_style: Option<String>,
    pub pane_border_hover_style: Option<String>,
    pub pane_border_status: Option<String>,
    pub pane_border_format: Option<String>,
    pub pane_border_lines: Option<String>,
    /// copy-mode-line-numbers option (off/default/absolute/relative/hybrid)
    pub copy_mode_line_numbers: Option<String>,
    /// Active pane scrollback size, for absolute/hybrid line numbers.
    pub copy_hsize: usize,
    pub copy_mode_line_number_style: Option<String>,
    pub copy_mode_current_line_number_style: Option<String>,
    /// window-status-format (short key to save bandwidth)
    pub wsf: Option<String>,
    /// window-status-current-format
    pub wscf: Option<String>,
    /// window-status-separator
    pub wss: Option<String>,
    /// window-status-style
    pub ws_style: Option<String>,
    /// window-status-current-style
    pub wsc_style: Option<String>,
    /// #451: status-left-style (was dropped in modularization)
    pub status_left_style: Option<String>,
    /// #451: status-right-style
    pub status_right_style: Option<String>,
    /// #451: window-status-activity-style
    pub wsa_style: Option<String>,
    /// #451: window-status-bell-style
    pub wsb_style: Option<String>,
    /// #451: window-status-last-style
    pub wsl_style: Option<String>,
    /// clock-mode active
    pub clock_mode: bool,
    /// clock-mode-colour (tmux option)
    pub clock_colour: Option<String>,
    /// Dynamic key bindings from server
    pub bindings: Vec<BindingEntry>,
    /// When true, hardcoded default keybindings are suppressed (set by unbind-key -a)
    pub defaults_suppressed: bool,
    /// scroll-enter-copy-mode option (mirror of server-side AppState field).
    /// When false, root key bindings that enter copy mode (e.g. PageUp ->
    /// copy-mode -u) are skipped so the key reaches the PTY (#284).
    pub scroll_enter_copy_mode: bool,
    /// pwsh-mouse-selection option (mirror of server-side AppState field)
    pub pwsh_mouse_selection: bool,
    /// mouse-selection option (mirror of server-side AppState field).
    /// When false, client suppresses its own drag-selection overlay so
    /// in-pane apps (opencode, etc.) can do their own mouse selection.
    pub mouse_selection: bool,
    /// paste-detection option (mirror of server-side AppState field)
    pub paste_detection: bool,
    /// choose-tree-preview option: when true, choose-session and
    /// choose-tree pickers open with the live preview pane visible.
    pub choose_tree_preview: bool,
    /// status-left-length (max display width for left status)
    pub status_left_length: usize,
    /// status-right-length (max display width for right status)
    pub status_right_length: usize,
    /// Number of status bar lines
    pub status_lines: usize,
    /// Custom format strings for additional status lines
    pub status_format: Vec<String>,
    /// mode-style for copy mode selection highlighting
    pub mode_style: Option<String>,
    /// message-style for the status-line message bar (display-message,
    /// command prompt). #372: previously never sent, so the client
    /// hard-coded bg=yellow,fg=black and ignored the user's option.
    pub message_style: Option<String>,
    /// status-position: "top" or "bottom"
    pub status_position: Option<String>,
    /// status-justify: "left", "centre", or "right"
    pub status_justify: Option<String>,
    /// Whether the status bar is visible (true) or hidden (false).
    /// Corresponds to `set-option status on/off`.
    pub status_visible: bool,
    /// Configured cursor style as DECSCUSR code (0-6) from server.
    /// Used as fallback when no child process has set a cursor shape.
    pub cursor_style_code: Option<u8>,
    /// One-shot clipboard text (base64-encoded) for OSC 52 delivery.
    pub clipboard_osc52: Option<String>,
    /// One-shot bell flag: server signals client to emit \x07 to the host terminal.
    pub bell: bool,
    /// set-titles: server pushes the expanded set-titles-string here when
    /// `set-titles on`.  Client emits OSC 0 to its host terminal whenever
    /// this value changes so external terminal tabs (Windows Terminal,
    /// iTerm2, etc.) follow the active pane / window title.
    pub host_title: Option<String>,
    /// Issue #269: OSC 9;4 progress indicator from the active pane,
    /// formatted as "<state>;<value>".  Client emits OSC 9;4 to its host
    /// terminal so apps inside a pane (Copilot CLI, build tools) keep
    /// driving the Windows Terminal taskbar / tab progress indicator.
    pub host_progress: Option<String>,
    /// Repeat key timeout in ms (default: 500, synced from server)
    pub repeat_time: u64,
    /// bold-is-bright option (issue #425): controls whether the console
    /// writer rewrites crossterm's 256-indexed basic colors to standard SGR.
    /// The writer lives in this client process, so the value is synced from
    /// the server and pushed into the writer's atomic each frame.
    pub bold_is_bright: bool,
    /// Whether a pane is currently zoomed (borders should be hidden)
    pub zoomed: bool,
    // ── Server-side overlay state ──
    /// Popup overlay active
    pub popup_active: bool,
    pub popup_command: Option<String>,
    pub popup_width: Option<u16>,
    pub popup_height: Option<u16>,
    pub popup_lines: Vec<String>,
    pub popup_rows: Vec<RowRunsJson>,
    pub popup_has_pty: bool,
    /// Cursor of the process running inside a PTY popup, relative to the
    /// popup's inner (inside-the-border) area.
    pub popup_cursor_row: Option<u16>,
    pub popup_cursor_col: Option<u16>,
    pub popup_hide_cursor: bool,
    /// Floating panes (tmux new-pane) overlaid on the active window.
    pub floats: Vec<FloatJson>,
    /// Confirm overlay active
    pub confirm_active: bool,
    pub confirm_prompt: Option<String>,
    /// Menu overlay active
    pub menu_active: bool,
    pub menu_title: Option<String>,
    pub menu_selected: usize,
    pub menu_items: Vec<ServerMenuItem>,
    /// Display-panes overlay active
    pub display_panes: bool,
    /// Pane base index for display-panes numbering
    pub pane_base_index: usize,
    /// Status bar message from display-message (without -p)
    pub status_message: Option<String>,
    /// Customize-mode overlay active
    pub customize_active: bool,
    pub customize_selected: usize,
    pub customize_scroll: usize,
    pub customize_editing: bool,
    pub customize_cursor: usize,
    pub customize_edit_buf: Option<String>,
    pub customize_filter: Option<String>,
    pub customize_options: Vec<CustomizeOption>,
}

impl ToJson for DumpState {
    fn to_json(&self) -> Value {
        Value::Object(vec![
            ("layout".into(), self.layout.to_json()),
            ("windows".into(), self.windows.to_json()),
            ("prefix".into(), self.prefix.to_json()),
            ("prefix2".into(), self.prefix2.to_json()),
            ("tree".into(), self.tree.to_json()),
            ("base_index".into(), self.base_index.to_json()),
            ("prediction_dimming".into(), self.prediction_dimming.to_json()),
            ("status_style".into(), self.status_style.to_json()),
            ("status_left".into(), self.status_left.to_json()),
            ("status_right".into(), self.status_right.to_json()),
            ("pane_border_style".into(), self.pane_border_style.to_json()),
            ("pane_active_border_style".into(), self.pane_active_border_style.to_json()),
            ("pane_border_hover_style".into(), self.pane_border_hover_style.to_json()),
            ("pane_border_status".into(), self.pane_border_status.to_json()),
            ("pane_border_format".into(), self.pane_border_format.to_json()),
            ("pane_border_lines".into(), self.pane_border_lines.to_json()),
            ("copy_mode_line_numbers".into(), self.copy_mode_line_numbers.to_json()),
            ("copy_hsize".into(), self.copy_hsize.to_json()),
            ("copy_mode_line_number_style".into(), self.copy_mode_line_number_style.to_json()),
            ("copy_mode_current_line_number_style".into(), self.copy_mode_current_line_number_style.to_json()),
            ("wsf".into(), self.wsf.to_json()),
            ("wscf".into(), self.wscf.to_json()),
            ("wss".into(), self.wss.to_json()),
            ("ws_style".into(), self.ws_style.to_json()),
            ("wsc_style".into(), self.wsc_style.to_json()),
            ("status_left_style".into(), self.status_left_style.to_json()),
            ("status_right_style".into(), self.status_right_style.to_json()),
            ("wsa_style".into(), self.wsa_style.to_json()),
            ("wsb_style".into(), self.wsb_style.to_json()),
            ("wsl_style".into(), self.wsl_style.to_json()),
            ("clock_mode".into(), self.clock_mode.to_json()),
            ("clock_colour".into(), self.clock_colour.to_json()),
            ("bindings".into(), self.bindings.to_json()),
            ("defaults_suppressed".into(), self.defaults_suppressed.to_json()),
            ("scroll_enter_copy_mode".into(), self.scroll_enter_copy_mode.to_json()),
            ("pwsh_mouse_selection".into(), self.pwsh_mouse_selection.to_json()),
            ("mouse_selection".into(), self.mouse_selection.to_json()),
            ("paste_detection".into(), self.paste_detection.to_json()),
            ("choose_tree_preview".into(), self.choose_tree_preview.to_json()),
            ("status_left_length".into(), self.status_left_length.to_json()),
            ("status_right_length".into(), self.status_right_length.to_json()),
            ("status_lines".into(), self.status_lines.to_json()),
            ("status_format".into(), self.status_format.to_json()),
            ("mode_style".into(), self.mode_style.to_json()),
            ("message_style".into(), self.message_style.to_json()),
            ("status_position".into(), self.status_position.to_json()),
            ("status_justify".into(), self.status_justify.to_json()),
            ("status_visible".into(), self.status_visible.to_json()),
            ("cursor_style_code".into(), self.cursor_style_code.to_json()),
            ("clipboard_osc52".into(), self.clipboard_osc52.to_json()),
            ("bell".into(), self.bell.to_json()),
            ("host_title".into(), self.host_title.to_json()),
            ("host_progress".into(), self.host_progress.to_json()),
            ("repeat_time".into(), self.repeat_time.to_json()),
            ("bold_is_bright".into(), self.bold_is_bright.to_json()),
            ("zoomed".into(), self.zoomed.to_json()),
            ("popup_active".into(), self.popup_active.to_json()),
            ("popup_command".into(), self.popup_command.to_json()),
            ("popup_width".into(), self.popup_width.to_json()),
            ("popup_height".into(), self.popup_height.to_json()),
            ("popup_lines".into(), self.popup_lines.to_json()),
            ("popup_rows".into(), self.popup_rows.to_json()),
            ("popup_has_pty".into(), self.popup_has_pty.to_json()),
            ("popup_cursor_row".into(), self.popup_cursor_row.to_json()),
            ("popup_cursor_col".into(), self.popup_cursor_col.to_json()),
            ("popup_hide_cursor".into(), self.popup_hide_cursor.to_json()),
            ("floats".into(), self.floats.to_json()),
            ("confirm_active".into(), self.confirm_active.to_json()),
            ("confirm_prompt".into(), self.confirm_prompt.to_json()),
            ("menu_active".into(), self.menu_active.to_json()),
            ("menu_title".into(), self.menu_title.to_json()),
            ("menu_selected".into(), self.menu_selected.to_json()),
            ("menu_items".into(), self.menu_items.to_json()),
            ("display_panes".into(), self.display_panes.to_json()),
            ("pane_base_index".into(), self.pane_base_index.to_json()),
            ("status_message".into(), self.status_message.to_json()),
            ("customize_active".into(), self.customize_active.to_json()),
            ("customize_selected".into(), self.customize_selected.to_json()),
            ("customize_scroll".into(), self.customize_scroll.to_json()),
            ("customize_editing".into(), self.customize_editing.to_json()),
            ("customize_cursor".into(), self.customize_cursor.to_json()),
            ("customize_edit_buf".into(), self.customize_edit_buf.to_json()),
            ("customize_filter".into(), self.customize_filter.to_json()),
            ("customize_options".into(), self.customize_options.to_json()),
        ])
    }
}

impl FromJson for DumpState {
    fn from_json(v: &Value) -> Result<Self, Error> {
        Ok(DumpState {
            layout: req(v, "layout")?,
            windows: req(v, "windows")?,
            prefix: opt(v, "prefix")?,
            prefix2: opt(v, "prefix2")?,
            tree: def(v, "tree")?,
            base_index: def_fn(v, "base_index", default_base_index)?,
            prediction_dimming: def_fn(v, "prediction_dimming", default_prediction_dimming)?,
            status_style: opt(v, "status_style")?,
            status_left: opt(v, "status_left")?,
            status_right: opt(v, "status_right")?,
            pane_border_style: opt(v, "pane_border_style")?,
            pane_active_border_style: opt(v, "pane_active_border_style")?,
            pane_border_hover_style: opt(v, "pane_border_hover_style")?,
            pane_border_status: opt(v, "pane_border_status")?,
            pane_border_format: opt(v, "pane_border_format")?,
            pane_border_lines: opt(v, "pane_border_lines")?,
            copy_mode_line_numbers: opt(v, "copy_mode_line_numbers")?,
            copy_hsize: def(v, "copy_hsize")?,
            copy_mode_line_number_style: opt(v, "copy_mode_line_number_style")?,
            copy_mode_current_line_number_style: opt(v, "copy_mode_current_line_number_style")?,
            wsf: opt(v, "wsf")?,
            wscf: opt(v, "wscf")?,
            wss: opt(v, "wss")?,
            ws_style: opt(v, "ws_style")?,
            wsc_style: opt(v, "wsc_style")?,
            status_left_style: opt(v, "status_left_style")?,
            status_right_style: opt(v, "status_right_style")?,
            wsa_style: opt(v, "wsa_style")?,
            wsb_style: opt(v, "wsb_style")?,
            wsl_style: opt(v, "wsl_style")?,
            clock_mode: def(v, "clock_mode")?,
            clock_colour: opt(v, "clock_colour")?,
            bindings: def(v, "bindings")?,
            defaults_suppressed: def(v, "defaults_suppressed")?,
            scroll_enter_copy_mode: def_fn(v, "scroll_enter_copy_mode", default_scroll_enter_copy_mode)?,
            pwsh_mouse_selection: def(v, "pwsh_mouse_selection")?,
            mouse_selection: def_fn(v, "mouse_selection", default_mouse_selection)?,
            paste_detection: def_fn(v, "paste_detection", default_paste_detection)?,
            choose_tree_preview: def(v, "choose_tree_preview")?,
            status_left_length: def_fn(v, "status_left_length", default_status_left_length)?,
            status_right_length: def_fn(v, "status_right_length", default_status_right_length)?,
            status_lines: def_fn(v, "status_lines", default_status_lines)?,
            status_format: def(v, "status_format")?,
            mode_style: opt(v, "mode_style")?,
            message_style: opt(v, "message_style")?,
            status_position: opt(v, "status_position")?,
            status_justify: opt(v, "status_justify")?,
            status_visible: def_fn(v, "status_visible", default_status_visible)?,
            cursor_style_code: opt(v, "cursor_style_code")?,
            clipboard_osc52: opt(v, "clipboard_osc52")?,
            bell: def(v, "bell")?,
            host_title: opt(v, "host_title")?,
            host_progress: opt(v, "host_progress")?,
            repeat_time: def_fn(v, "repeat_time", default_repeat_time)?,
            bold_is_bright: def_fn(v, "bold_is_bright", default_bold_is_bright)?,
            zoomed: def(v, "zoomed")?,
            popup_active: def(v, "popup_active")?,
            popup_command: opt(v, "popup_command")?,
            popup_width: opt(v, "popup_width")?,
            popup_height: opt(v, "popup_height")?,
            popup_lines: def(v, "popup_lines")?,
            popup_rows: def(v, "popup_rows")?,
            popup_has_pty: def(v, "popup_has_pty")?,
            popup_cursor_row: opt(v, "popup_cursor_row")?,
            popup_cursor_col: opt(v, "popup_cursor_col")?,
            popup_hide_cursor: def(v, "popup_hide_cursor")?,
            floats: def(v, "floats")?,
            confirm_active: def(v, "confirm_active")?,
            confirm_prompt: opt(v, "confirm_prompt")?,
            menu_active: def(v, "menu_active")?,
            menu_title: opt(v, "menu_title")?,
            menu_selected: def(v, "menu_selected")?,
            menu_items: def(v, "menu_items")?,
            display_panes: def(v, "display_panes")?,
            pane_base_index: def(v, "pane_base_index")?,
            status_message: opt(v, "status_message")?,
            customize_active: def(v, "customize_active")?,
            customize_selected: def(v, "customize_selected")?,
            customize_scroll: def(v, "customize_scroll")?,
            customize_editing: def(v, "customize_editing")?,
            customize_cursor: def(v, "customize_cursor")?,
            customize_edit_buf: opt(v, "customize_edit_buf")?,
            customize_filter: opt(v, "customize_filter")?,
            customize_options: def(v, "customize_options")?,
        })
    }
}
