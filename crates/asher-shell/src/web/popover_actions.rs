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
        self.sync_chrome();
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
        self.sync_chrome();
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
        self.sync_chrome();
        self.sync_surfaces();
        self.surfaces.quick.set_visible(true);
        self.surfaces.date.set_visible(false);
        self.surfaces.start_menu.set_visible(false);
    }

    pub(super) fn close_quick_settings(&mut self) {
        if !self.quick_visible {
            return;
        }
        self.quick_visible = false;
        self.sync_chrome();
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
        self.sync_chrome();
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
        self.sync_chrome();
        self.sync_surfaces();
        self.surfaces.date.set_visible(false);
    }

    pub(super) fn close_transient_popovers(&mut self) {
        self.quick_visible = false;
        self.date_visible = false;
        self.start_menu_visible = false;
        self.close_panel_menu();
        self.sync_chrome();
        self.sync_surfaces();
        self.surfaces.quick.set_visible(false);
        self.surfaces.date.set_visible(false);
        self.surfaces.start_menu.set_visible(false);
    }
}
