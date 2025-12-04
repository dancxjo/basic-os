pub struct AppSpec {
    pub name: &'static str,
    pub bin_name: &'static str,
    pub autostart: bool,
    pub show_in_graph_viewer: bool,
}

pub static APPS: &[AppSpec] = &[
    AppSpec {
        name: "init",
        bin_name: "init",
        autostart: false, // init is already running
        show_in_graph_viewer: false,
    },
    AppSpec {
        name: "widget_host",
        bin_name: "widget_host",
        autostart: true,
        show_in_graph_viewer: false,
    },
    AppSpec {
        name: "compositor",
        bin_name: "compositor",
        autostart: true,
        show_in_graph_viewer: false,
    },
    AppSpec {
        name: "demo_app",
        bin_name: "demo_app",
        autostart: true,
        show_in_graph_viewer: true,
    },
    AppSpec {
        name: "prefs_demo",
        bin_name: "prefs_demo",
        autostart: true,
        show_in_graph_viewer: true,
    },
    AppSpec {
        name: "thing_viewer",
        bin_name: "thing_viewer",
        autostart: false,
        show_in_graph_viewer: true,
    },
    AppSpec {
        name: "text_editor",
        bin_name: "text_editor",
        autostart: false,
        show_in_graph_viewer: true,
    },
    AppSpec {
        name: "self_editing_demo",
        bin_name: "self_editing_demo",
        autostart: false,
        show_in_graph_viewer: true,
    },
    AppSpec {
        name: "graph_viewer",
        bin_name: "graph_viewer",
        autostart: true,
        show_in_graph_viewer: false,
    },
    AppSpec {
        name: "widget_host",
        bin_name: "widget_host",
        autostart: true,
        show_in_graph_viewer: false,
    },
];
