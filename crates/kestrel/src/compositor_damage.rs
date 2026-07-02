use crate::{
    damage::{
        DamageTracker, LayerGeometryTracker, blur_damage_elements, damage_elements,
        expand_damage_for_blur_targets, merge_damage_rectangles,
    },
    layers::{self, LayerRenderTarget},
    render::LayerElement,
    scene_blur::blur_sample_rect,
    window_clip::RoundedWindowElement,
};
use smithay::{
    backend::renderer::{
        element::{memory::MemoryRenderBufferRenderElement, surface::WaylandSurfaceRenderElement},
        gles::GlesRenderer,
    },
    output::Output,
    utils::{Physical, Rectangle, Size},
};

type LayerSurfaceElement = LayerElement;
type WindowSurfaceElement = WaylandSurfaceRenderElement<GlesRenderer>;
type MemoryElement = MemoryRenderBufferRenderElement<GlesRenderer>;
type WindowElement = RoundedWindowElement<WindowSurfaceElement>;

pub struct CompositorDamagePlan {
    pub damage: Vec<Rectangle<i32, Physical>>,
    pub blur_damage: Vec<Rectangle<i32, Physical>>,
}

pub struct CompositorDamageContext<'a> {
    pub output_size: Size<i32, Physical>,
    pub output: &'a Output,
    pub buffer_age: usize,
    pub force_full_damage: bool,
    pub blur_animating: bool,
    pub window_effect_targets: &'a [LayerRenderTarget],
    pub top_targets: &'a [LayerRenderTarget],
    pub overlay_targets: &'a [LayerRenderTarget],
    pub background: Option<&'a MemoryElement>,
    pub background_layer: &'a [LayerSurfaceElement],
    pub bottom_layer: &'a [LayerSurfaceElement],
    pub windows: &'a [WindowElement],
    pub window_chrome: &'a [MemoryElement],
    pub top_layer: &'a [LayerSurfaceElement],
    pub overlay_layer: &'a [LayerSurfaceElement],
    pub loading: Option<&'a MemoryElement>,
}

pub fn plan_compositor_damage(
    ctx: CompositorDamageContext<'_>,
    damage_tracker: &mut DamageTracker,
    blur_damage_tracker: &mut DamageTracker,
    layer_geometry: &mut LayerGeometryTracker,
) -> CompositorDamagePlan {
    let force_damage = ctx.force_full_damage;
    let damage_plan = {
        let elements = damage_elements(
            ctx.background,
            ctx.background_layer,
            ctx.bottom_layer,
            ctx.windows,
            ctx.window_chrome,
            ctx.top_layer,
            ctx.overlay_layer,
            ctx.loading,
        );
        damage_tracker.plan(ctx.output_size, ctx.buffer_age, force_damage, &elements)
    };
    let blur_damage_plan = {
        let elements = blur_damage_elements(
            ctx.background,
            ctx.background_layer,
            ctx.bottom_layer,
            ctx.windows,
        );
        blur_damage_tracker.plan(ctx.output_size, ctx.buffer_age, force_damage, &elements)
    };
    let blur_animation_damage = if ctx.blur_animating {
        blur_target_rectangles(
            ctx.output_size,
            &[
                ctx.window_effect_targets,
                ctx.top_targets,
                ctx.overlay_targets,
            ],
        )
    } else {
        Vec::new()
    };
    let mut damage_rectangles = damage_plan.rectangles.clone();
    damage_rectangles.extend(blur_animation_damage.iter().copied());
    let mut blur_damage_rectangles = blur_damage_plan.rectangles.clone();
    blur_damage_rectangles.extend(blur_animation_damage);
    let damage = expand_damage_for_blur_targets(
        ctx.output_size,
        &damage_rectangles,
        &blur_damage_rectangles,
        &[
            ctx.window_effect_targets,
            ctx.top_targets,
            ctx.overlay_targets,
        ],
    );
    let (damage, geometry_changed) = layer_geometry.expand_damage(
        ctx.output_size,
        &damage,
        &layers::layer_surface_rects(ctx.output),
    );
    let blur_damage = expand_damage_for_blur_targets(
        ctx.output_size,
        &blur_damage_rectangles,
        &damage,
        &[
            ctx.window_effect_targets,
            ctx.top_targets,
            ctx.overlay_targets,
        ],
    );
    let blur_damage = merge_damage_rectangles(
        Rectangle::<i32, Physical>::from_size(ctx.output_size),
        blur_damage
            .into_iter()
            .chain(damage.iter().copied())
            .collect(),
    );
    let force_geometry_damage = geometry_changed;
    let blur_damage = if force_geometry_damage {
        vec![Rectangle::<i32, Physical>::from_size(ctx.output_size)]
    } else {
        blur_damage
    };

    CompositorDamagePlan {
        damage,
        blur_damage,
    }
}

fn blur_target_rectangles(
    output_size: Size<i32, Physical>,
    target_groups: &[&[LayerRenderTarget]],
) -> Vec<Rectangle<i32, Physical>> {
    let output = Rectangle::<i32, Physical>::from_size(output_size);
    merge_damage_rectangles(
        output,
        target_groups
            .iter()
            .flat_map(|targets| targets.iter())
            .filter_map(|target| blur_sample_rect(output_size, target))
            .collect(),
    )
}
