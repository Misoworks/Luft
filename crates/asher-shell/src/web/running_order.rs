use crate::ipc::ShellModel;

pub fn from_model(model: &ShellModel) -> Vec<asher_ipc::WindowId> {
    let mut order = Vec::new();
    sync(&mut order, model);
    order
}

pub fn sync(order: &mut Vec<asher_ipc::WindowId>, model: &ShellModel) {
    order.retain(|id| model.windows.iter().any(|window| window.id == *id));
    for window in &model.windows {
        if !order.contains(&window.id) {
            order.push(window.id);
        }
    }
}
