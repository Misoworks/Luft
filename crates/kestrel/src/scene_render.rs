use crate::{
    layers::{BlurLayer, LayerRenderTarget},
    render::{LayerElement, window_chrome_elements_for_window},
    scene_blur::{self, BlurElement, SceneBlurCache},
    state::KestrelState,
    window::ManagedWindow,
    window_clip::{RoundedWindowElement, window_elements_for_window},
};
use asher_layout::WorkspaceId;
use smithay::{
    backend::renderer::{
        Color32F, Frame, Renderer, RendererSuper,
        element::{memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement},
        gles::{GlesError, GlesRenderer, GlesTarget},
        utils::draw_render_elements,
    },
    utils::{Physical, Rectangle, Size, Transform},
};

type LayerSurfaceElement = LayerElement;
type WindowSurfaceElement = WaylandSurfaceRenderElement<GlesRenderer>;
type MemoryElement = MemoryRenderBufferRenderElement<GlesRenderer>;
type WindowElement = RoundedWindowElement<WindowSurfaceElement>;

pub struct SceneRenderRequest<'a> {
    pub state: &'a KestrelState,
    pub output_size: Size<i32, Physical>,
    pub target_transform: Transform,
    pub damage: &'a [Rectangle<i32, Physical>],
    pub blur_damage: &'a [Rectangle<i32, Physical>],
    pub blur_enabled: bool,
    pub background: Option<MemoryElement>,
    pub background_layer: &'a [LayerSurfaceElement],
    pub bottom_layer: &'a [LayerSurfaceElement],
    pub windows: &'a [WindowElement],
    pub window_chrome: &'a [MemoryElement],
    pub window_targets: &'a [LayerRenderTarget],
    pub top_targets: &'a [LayerRenderTarget],
    pub top_layer: &'a [LayerSurfaceElement],
    pub overlay_targets: &'a [LayerRenderTarget],
    pub overlay_layer: &'a [LayerSurfaceElement],
    pub loading: Option<MemoryElement>,
    pub debug: Option<MemoryElement>,
}

fn draw_blur_elements(
    frame: &mut <GlesRenderer as RendererSuper>::Frame<'_, '_>,
    elements: &[BlurElement],
    damage: &[Rectangle<i32, Physical>],
) -> Result<(), GlesError> {
    for element in elements.iter().rev() {
        draw_render_elements::<GlesRenderer, f64, BlurElement>(
            frame,
            1.0,
            std::slice::from_ref(element),
            damage,
        )?;
    }
    Ok(())
}

pub fn render_scene(
    blur_cache: &mut SceneBlurCache,
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    request: SceneRenderRequest<'_>,
) -> Result<(), GlesError> {
    if !request.blur_enabled
        || (request.window_targets.is_empty()
            && request.top_targets.is_empty()
            && request.overlay_targets.is_empty()
            && !blur_cache.has_cached_elements())
    {
        return render_flat_scene(renderer, framebuffer, request);
    }

    if request.window_targets.is_empty()
        && !blur_cache.targets_need_capture(
            request.output_size,
            request.top_targets,
            request.blur_damage,
        )
        && !blur_cache.targets_need_capture(
            request.output_size,
            request.overlay_targets,
            request.blur_damage,
        )
    {
        render_flat_scene_with_cached_layer_blur(blur_cache, renderer, framebuffer, request)
    } else {
        render_staged_scene(blur_cache, renderer, framebuffer, request)
    }
}

fn render_flat_scene(
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    request: SceneRenderRequest<'_>,
) -> Result<(), GlesError> {
    let mut frame = renderer.render(framebuffer, request.output_size, request.target_transform)?;
    frame.clear(Color32F::new(0.08, 0.085, 0.09, 1.0), request.damage)?;
    draw_optional_memory(&mut frame, request.background.as_ref(), request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.background_layer, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.bottom_layer, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.windows, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.window_chrome, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.top_layer, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.overlay_layer, request.damage)?;
    draw_optional_memory(&mut frame, request.loading.as_ref(), request.damage)?;
    draw_optional_memory(&mut frame, request.debug.as_ref(), request.damage)?;
    let _ = frame.finish()?;
    Ok(())
}

fn render_flat_scene_with_cached_layer_blur(
    blur_cache: &SceneBlurCache,
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    request: SceneRenderRequest<'_>,
) -> Result<(), GlesError> {
    let top_blur = blur_cache.cached_elements(
        renderer,
        request.output_size,
        request.target_transform,
        BlurLayer::Top,
        request.top_targets,
    )?;
    let overlay_blur = blur_cache.cached_elements(
        renderer,
        request.output_size,
        request.target_transform,
        BlurLayer::Overlay,
        request.overlay_targets,
    )?;
    let top_blur_damage = blur_target_damage(request.output_size, request.top_targets);
    let overlay_blur_damage = blur_target_damage(request.output_size, request.overlay_targets);
    let mut frame = renderer.render(framebuffer, request.output_size, request.target_transform)?;
    frame.clear(Color32F::new(0.08, 0.085, 0.09, 1.0), request.damage)?;
    draw_optional_memory(&mut frame, request.background.as_ref(), request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.background_layer, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.bottom_layer, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.windows, request.damage)?;
    draw_render_elements(&mut frame, 1.0, request.window_chrome, request.damage)?;
    draw_blur_elements(&mut frame, &top_blur, &top_blur_damage)?;
    draw_render_elements(&mut frame, 1.0, request.top_layer, request.damage)?;
    draw_blur_elements(&mut frame, &overlay_blur, &overlay_blur_damage)?;
    draw_render_elements(&mut frame, 1.0, request.overlay_layer, request.damage)?;
    draw_optional_memory(&mut frame, request.loading.as_ref(), request.damage)?;
    draw_optional_memory(&mut frame, request.debug.as_ref(), request.damage)?;
    let _ = frame.finish()?;
    Ok(())
}

fn render_staged_scene(
    blur_cache: &mut SceneBlurCache,
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    request: SceneRenderRequest<'_>,
) -> Result<(), GlesError> {
    let blur_quality = request.state.config.effects.blur_quality;
    {
        let mut frame =
            renderer.render(framebuffer, request.output_size, request.target_transform)?;
        frame.clear(Color32F::new(0.08, 0.085, 0.09, 1.0), request.damage)?;
        draw_optional_memory(&mut frame, request.background.as_ref(), request.damage)?;
        draw_render_elements(&mut frame, 1.0, request.background_layer, request.damage)?;
        let _ = frame.finish()?;
    }

    let mut batched_windows = Vec::new();
    let mut batched_chrome = Vec::new();
    for entry in scene_window_entries(request.state) {
        let targets = targets_for_window(request.window_targets, entry.window);
        let window =
            window_elements_for_window(renderer, entry.window, entry.offset_x, request.output_size);
        let chrome = window_chrome_elements_for_window(
            renderer,
            request.state,
            entry.window,
            entry.offset_x,
        )?;
        if targets.is_empty() {
            batched_windows.extend(window);
            batched_chrome.extend(chrome);
            continue;
        }

        flush_window_batch(
            renderer,
            framebuffer,
            request.output_size,
            request.target_transform,
            request.damage,
            &mut batched_windows,
            &mut batched_chrome,
        )?;
        let blur = scene_blur::capture_blur_elements(
            blur_cache,
            renderer,
            framebuffer,
            request.output_size,
            request.target_transform,
            &targets,
            request.blur_damage,
            request.blur_enabled,
            blur_quality,
        )?;
        let mut frame =
            renderer.render(framebuffer, request.output_size, request.target_transform)?;
        let blur_damage = blur_target_damage(request.output_size, &targets);
        draw_blur_elements(&mut frame, &blur, &blur_damage)?;
        draw_render_elements(&mut frame, 1.0, &window, request.damage)?;
        draw_render_elements(&mut frame, 1.0, &chrome, request.damage)?;
        let _ = frame.finish()?;
    }
    flush_window_batch(
        renderer,
        framebuffer,
        request.output_size,
        request.target_transform,
        request.damage,
        &mut batched_windows,
        &mut batched_chrome,
    )?;

    let top_blur = scene_blur::capture_blur_elements(
        blur_cache,
        renderer,
        framebuffer,
        request.output_size,
        request.target_transform,
        request.top_targets,
        request.blur_damage,
        request.blur_enabled,
        blur_quality,
    )?;
    let overlay_blur = scene_blur::capture_blur_elements(
        blur_cache,
        renderer,
        framebuffer,
        request.output_size,
        request.target_transform,
        request.overlay_targets,
        request.blur_damage,
        request.blur_enabled,
        blur_quality,
    )?;
    {
        let mut frame =
            renderer.render(framebuffer, request.output_size, request.target_transform)?;
        let blur_damage = blur_target_damage(request.output_size, request.top_targets);
        draw_render_elements(&mut frame, 1.0, request.bottom_layer, request.damage)?;
        draw_blur_elements(&mut frame, &top_blur, &blur_damage)?;
        draw_render_elements(&mut frame, 1.0, request.top_layer, request.damage)?;
        let _ = frame.finish()?;
    }

    let mut frame = renderer.render(framebuffer, request.output_size, request.target_transform)?;
    let blur_damage = blur_target_damage(request.output_size, request.overlay_targets);
    draw_blur_elements(&mut frame, &overlay_blur, &blur_damage)?;
    draw_render_elements(&mut frame, 1.0, request.overlay_layer, request.damage)?;
    draw_optional_memory(&mut frame, request.loading.as_ref(), request.damage)?;
    draw_optional_memory(&mut frame, request.debug.as_ref(), request.damage)?;
    let _ = frame.finish()?;

    Ok(())
}

fn flush_window_batch(
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    output_size: Size<i32, Physical>,
    target_transform: Transform,
    damage: &[Rectangle<i32, Physical>],
    windows: &mut Vec<WindowElement>,
    chrome: &mut Vec<MemoryElement>,
) -> Result<(), GlesError> {
    if windows.is_empty() && chrome.is_empty() {
        return Ok(());
    }

    windows.reverse();
    chrome.reverse();
    let mut frame = renderer.render(framebuffer, output_size, target_transform)?;
    draw_render_elements(&mut frame, 1.0, windows.as_slice(), damage)?;
    draw_render_elements(&mut frame, 1.0, chrome.as_slice(), damage)?;
    let _ = frame.finish()?;
    windows.clear();
    chrome.clear();
    Ok(())
}

fn draw_optional_memory(
    frame: &mut <GlesRenderer as RendererSuper>::Frame<'_, '_>,
    element: Option<&MemoryElement>,
    damage: &[Rectangle<i32, Physical>],
) -> Result<(), GlesError> {
    if let Some(element) = element {
        draw_render_elements(frame, 1.0, std::slice::from_ref(element), damage)?;
    }

    Ok(())
}

fn blur_target_damage(
    output_size: Size<i32, Physical>,
    targets: &[LayerRenderTarget],
) -> Vec<Rectangle<i32, Physical>> {
    let output = Rectangle::<i32, Physical>::from_size(output_size);
    targets
        .iter()
        .filter_map(|target| {
            Rectangle::<i32, Physical>::new(
                (target.location.x, target.location.y).into(),
                (target.size.w, target.size.h).into(),
            )
            .intersection(output)
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct SceneWindowEntry<'a> {
    window: &'a ManagedWindow,
    offset_x: i32,
}

fn scene_window_entries(state: &KestrelState) -> Vec<SceneWindowEntry<'_>> {
    let mut entries = Vec::new();
    if let Some(transition) = state.workspace_transition() {
        let width = state.output_size().w as f64;
        let direction = transition.direction as f64;
        let from_offset = (-direction * width * transition.progress).round() as i32;
        let to_offset = (direction * width * (1.0 - transition.progress)).round() as i32;
        append_workspace_entries(state, &transition.from, from_offset, &mut entries);
        append_workspace_entries(state, &transition.to, to_offset, &mut entries);
    } else {
        append_workspace_entries(state, state.layout.active_workspace(), 0, &mut entries);
    }
    entries.reverse();
    entries
}

fn append_workspace_entries<'a>(
    state: &'a KestrelState,
    workspace: &WorkspaceId,
    offset_x: i32,
    entries: &mut Vec<SceneWindowEntry<'a>>,
) {
    entries.extend(
        state
            .windows
            .render_windows_on_workspace(workspace)
            .map(|window| SceneWindowEntry { window, offset_x }),
    );
}

fn targets_for_window(
    targets: &[LayerRenderTarget],
    window: &ManagedWindow,
) -> Vec<LayerRenderTarget> {
    let surface = window.surface.wl_surface();
    targets
        .iter()
        .filter(|target| &target.surface == surface)
        .cloned()
        .collect()
}
