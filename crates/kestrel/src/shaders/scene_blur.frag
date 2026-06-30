#version 100
#extension GL_OES_standard_derivatives : enable

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
uniform vec2 texel;
uniform vec2 target_size;
uniform float radius;
uniform float shape;
uniform vec2 direction;
uniform float final_pass;
uniform float mask_pass;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

float hash(vec2 value) {
    return fract(sin(dot(value, vec2(127.1, 311.7))) * 43758.5453123) - 0.5;
}

float sdfRoundedBox(vec2 position, vec2 center, vec2 extents, float corner_radius) {
    vec2 p = position - center;
    vec2 q = abs(p) - extents + vec2(corner_radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - corner_radius;
}

float roundedCoverage(vec2 pixel) {
    if (radius <= 0.0) {
        return 1.0;
    }

    vec2 center = target_size * 0.5;
    float distance = sdfRoundedBox(pixel + vec2(0.5), center, center, radius);
    return distance <= 0.0 ? 1.0 : 0.0;
}

float cornerCoverage(vec2 pixel, vec2 center) {
    return distance(pixel + vec2(0.5), center) <= radius ? 1.0 : 0.0;
}

float leftRoundedCoverage(vec2 pixel) {
    if (radius <= 0.0 || pixel.x >= radius || (pixel.y >= radius && pixel.y < target_size.y - radius)) {
        return 1.0;
    }

    vec2 center = vec2(radius, pixel.y < radius ? radius : target_size.y - radius);
    return cornerCoverage(pixel, center);
}

float rightRoundedCoverage(vec2 pixel) {
    if (radius <= 0.0 || pixel.x < target_size.x - radius || (pixel.y >= radius && pixel.y < target_size.y - radius)) {
        return 1.0;
    }

    vec2 center = vec2(target_size.x - radius, pixel.y < radius ? radius : target_size.y - radius);
    return cornerCoverage(pixel, center);
}

float materialCoverage(vec2 pixel) {
    if (shape < 0.5) {
        return 1.0;
    }
    if (shape < 1.5) {
        return roundedCoverage(pixel);
    }
    if (shape < 2.5) {
        return leftRoundedCoverage(pixel);
    }
    return rightRoundedCoverage(pixel);
}

vec2 blurSampleUv(vec2 uv) {
    return clamp(uv, vec2(0.0), vec2(1.0));
}

void main() {
    vec2 uv = v_coords;
    vec4 color;

    if (mask_pass > 0.5) {
        color = texture2D(tex, blurSampleUv(uv));
    } else {
        vec2 step = texel * direction;
        color = texture2D(tex, blurSampleUv(uv)) * 0.227027;
        color += texture2D(tex, blurSampleUv(uv + step * 1.384615)) * 0.316216;
        color += texture2D(tex, blurSampleUv(uv - step * 1.384615)) * 0.316216;
        color += texture2D(tex, blurSampleUv(uv + step * 3.230769)) * 0.070270;
        color += texture2D(tex, blurSampleUv(uv - step * 3.230769)) * 0.070270;

        if (final_pass < 0.5) {
            gl_FragColor = color;
            return;
        }
    }

    float luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    color.rgb = luma + (color.rgb - vec3(luma)) * 1.06;
    color.rgb = min(color.rgb * 1.12 + vec3(0.018), vec3(1.0));
    color.rgb += hash(uv * target_size) * 0.0028;

    float coverage = final_pass > 0.5 ? materialCoverage(uv * target_size) : 1.0;
    if (coverage <= 0.0) {
        discard;
    }
    color.a = coverage * alpha;
    color.rgb *= color.a;

#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif

    gl_FragColor = color;
}
