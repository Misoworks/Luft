use super::WebShell;

impl WebShell {
    pub(super) fn toggle_start_menu(&mut self) {
        if self.start_menu_visible {
            self.close_start_menu();
        } else {
            self.open_start_menu();
        }
    }

    pub(super) fn open_start_menu(&mut self) {
        self.quick_visible = false;
        self.date_visible = false;
        self.start_menu_visible = true;
        self.sync_surfaces();
        self.surfaces.quick.set_visible(false);
        self.surfaces.date.set_visible(false);
        self.surfaces.start_menu.set_visible(true);
    }

    pub(super) fn close_start_menu(&mut self) {
        if !self.start_menu_visible {
            return;
        }
        self.start_menu_visible = false;
        self.sync_surfaces();
        self.surfaces.start_menu.set_visible(false);
    }

    pub(super) fn toggle_quick_settings(&mut self) {
        if self.quick_visible {
            self.close_quick_settings();
        } else {
            self.open_quick_settings();
        }
    }

    pub(super) fn open_quick_settings(&mut self) {
        self.date_visible = false;
        self.start_menu_visible = false;
        self.quick_visible = true;
        self.refresh_status_now();
        self.sync_surfaces();
        self.surfaces.quick.set_visible(true);
        self.surfaces.date.set_visible(false);
        self.surfaces.start_menu.set_visible(false);
    }

    pub(super) fn close_quick_settings(&mut self) {
        if !self.quick_visible {
            return;
        }
        self.close_session_menu();
        self.quick_visible = false;
        self.sync_surfaces();
        self.surfaces.quick.set_visible(false);
    }

    pub(super) fn toggle_date_center(&mut self) {
        if self.date_visible {
            self.close_date_center();
        } else {
            self.open_date_center();
        }
    }

    pub(super) fn open_date_center(&mut self) {
        self.quick_visible = false;
        self.start_menu_visible = false;
        self.date_visible = true;
        self.sync_surfaces();
        self.surfaces.date.set_visible(true);
        self.surfaces.quick.set_visible(false);
        self.surfaces.start_menu.set_visible(false);
    }

    pub(super) fn close_date_center(&mut self) {
        if !self.date_visible {
            return;
        }
        self.date_visible = false;
        self.sync_surfaces();
        self.surfaces.date.set_visible(false);
    }

    pub(super) fn close_transient_popovers(&mut self) {
        self.quick_visible = false;
        self.date_visible = false;
        self.start_menu_visible = false;
        self.close_panel_menu();
        self.close_session_menu();
        self.sync_surfaces();
        self.surfaces.quick.set_visible(false);
        self.surfaces.date.set_visible(false);
        self.surfaces.start_menu.set_visible(false);
    }

    pub(super) fn toggle_session_menu(&mut self) {
        if self.session_menu_visible {
            self.close_session_menu();
        } else {
            self.open_session_menu();
        }
    }

    pub(super) fn open_session_menu(&mut self) {
        if !self.quick_visible {
            self.open_quick_settings();
        }
        if self.session_menu_visible {
            return;
        }
        let snapshot = super::model::WebShellSnapshot::from_shell(
            super::snapshot::WebShellSnapshotInput {
                model: &self.model,
                running_window_order: &self.running_app_order,
                status: &self.status,
                tray: self.tray.snapshot(),
                notifications: self.notifications.snapshot(),
                panel_apps: &self.panel_apps,
                panel_menu_command: self.panel_menu_command.as_deref(),
                panel_menu_x: self.panel_menu_x,
                applications: &self.applications,
                palette: self.palette,
                start_menu_open: self.start_menu_visible,
                quick_settings_open: self.quick_visible,
                date_center_open: self.date_visible,
            },
        );
        let qs_height = super::surface_sizing::quick_settings_size(&snapshot).1;
        self.session_menu_visible = true;
        self.session_menu_qs_height = Some(qs_height);
        self.surfaces.set_session_menu_qs_height(Some(qs_height));
        self.sync_surfaces();
        self.surfaces.set_session_menu_visible(true);
    }

    pub(super) fn close_session_menu(&mut self) {
        if !self.session_menu_visible {
            return;
        }
        self.session_menu_visible = false;
        self.session_menu_qs_height = None;
        self.surfaces.set_session_menu_qs_height(None);
        self.sync_surfaces();
        self.surfaces.set_session_menu_visible(false);
    }
}
