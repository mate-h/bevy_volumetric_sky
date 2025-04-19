#define_import_path atmosphere

struct AtmosphereSettings {
    sun_position: vec3<f32>,
    eye_position: vec3<f32>,
    sun_intensity: f32,
    rayleigh_scattering: vec3<f32>,
    mie_scattering: vec3<f32>,
    mie_g: f32,
    atmosphere_height: f32,
    cloud_coverage: f32,
    enable_clouds: f32,
    exposure: f32,
    multiple_scattering_factor: f32,
    enable_volumetric_shadows: f32,
    max_raymarch_samples: f32,
}
@group(0) @binding(0) var<uniform> uniformBuffer: AtmosphereSettings;

// LUTS
@group(0) @binding(1) var transmittanceTexture: texture_2d<f32>;
@group(0) @binding(2) var transmittanceTextureSampler: sampler;
@group(0) @binding(3) var multipleScatteringTexture: texture_2d<f32>;
@group(0) @binding(4) var multipleScatteringTextureSampler: sampler;
@group(0) @binding(5) var cloudTexture: texture_3d<f32>;
@group(0) @binding(6) var cloudTextureSampler: sampler;

#ifdef USE_DEPTH_BUFFER
#import bevy_render::view::View
@group(0) @binding(7)
var<uniform> view: View;
#endif

#ifdef USE_SHADOW_MAP
#import bevy_pbr::mesh_view_types::Lights;
@group(1) @binding(0) var directional_shadow_texture: texture_depth_2d_array;
@group(1) @binding(1) var directional_shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<uniform> lights: Lights;
// #import bevy_pbr::shadows::fetch_directional_shadow
fn sample_shadow_map_hardware(light_local: vec2<f32>, depth: f32, array_index: i32) -> f32 {
    return textureSampleCompareLevel(
        directional_shadow_texture,
        directional_shadow_sampler,
        light_local,
        array_index,
        depth,
    );
}

fn fetch_directional_shadow(light_index: u32, world_pos: vec4<f32>, normal: vec3<f32>, view_z: f32) -> f32 {
    let light = &lights.directional_lights[light_index]; // Using first directional light
    
    // Get cascade index based on view_z
    var cascade_index = 0u;
    for (var i = 0u; i < (*light).num_cascades; i++) {
        if (-view_z < (*light).cascades[i].far_bound) {
            cascade_index = i;
            break;
        }
    }
    
    // Get the cascade
    let cascade = &(*light).cascades[cascade_index];
    
    // Calculate position with bias
    let normal_offset = (*light).shadow_normal_bias * (*cascade).texel_size * normal;
    let depth_offset = (*light).shadow_depth_bias * (*light).direction_to_light;
    let offset_position = vec4<f32>(world_pos.xyz + normal_offset + depth_offset, world_pos.w);
    
    // Transform to light space
    let light_local = (*cascade).clip_from_world * offset_position;
    
    // Convert to UV coordinates
    let ndc = light_local.xyz / light_local.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    
    // Early exit if outside shadow map
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return 1.0;
    }
    
    let depth = ndc.z;
    let array_index = i32((*light).depth_texture_base_index + cascade_index);
    
    // Sample shadow map
    return textureSampleCompareLevel(
        directional_shadow_texture,
        directional_shadow_sampler,
        uv,
        array_index,
        depth
    );
}

fn getShadow(P: vec3<f32>, ray_dir: vec3<f32>) -> f32 {
    // For raymarched volumes, we can use the ray direction as the normal
    // since we don't have surface normals
    let world_normal = -ray_dir; // Point against ray direction
    let Atmosphere = GetAtmosphereParameters();
    let world_pos = vec4<f32>(P + vec3<f32>(0.0, -Atmosphere.BottomRadius, 0.0), 1.0);
    // Get view space Z coordinate for cascade selection
    let view_pos = view.view_from_world * world_pos;
    let view_z = view_pos.z;

    // get local up vector
    let local_up = vec3<f32>(0.0, 1.0, 0.0);

    // Assuming we're using the first directional light (index 0)
    return 1.0 - fetch_directional_shadow(0u, world_pos, world_normal, view_z);
}
#endif

var<private> PI_1_4: f32 = 0.78539816339744830961566084522142;
var<private> PI_1_2: f32 = 1.5707963267948966192313216916398;
var<private> PI: f32 = 3.1415926535897932384626433832795;
var<private> PI_2: f32 = 6.283185307179586476925286766559;
var<private> EPSILON: f32 = 0.0000001;

struct UvToLutResult {
    viewHeight: f32,
    viewZenithCosAngle: f32,
};

fn GetAtmosphereParameters() -> AtmosphereParameters {
    var info: AtmosphereParameters;

    let EarthBottomRadius: f32 = 6360.0;
    var scalar: f32 = 1.0; // TODO: control with uniform
    var EarthTopRadius: f32 = EarthBottomRadius + 100.0 * scalar;
    var EarthRayleighScaleHeight: f32 = 8.0 * scalar;
    var EarthMieScaleHeight: f32 = 1.2 * scalar;

    info.BottomRadius = EarthBottomRadius;
    info.TopRadius = EarthTopRadius;
    info.GroundAlbedo = vec3<f32>(0.0, 0.0, 0.0);

    info.RayleighDensityExpScale = -1.0 / EarthRayleighScaleHeight;
    info.RayleighScattering = vec3<f32>(0.005802, 0.013558, 0.033100);

    info.MieDensityExpScale = -1.0 / EarthMieScaleHeight;
    info.MieScattering = vec3<f32>(0.003996, 0.003996, 0.003996);
    info.MieExtinction = vec3<f32>(0.004440, 0.004440, 0.004440);
    info.MieAbsorption = info.MieExtinction - info.MieScattering;
    info.MiePhaseG = 0.8;

    info.AbsorptionDensity0LayerWidth = 25.0 * scalar;
    info.AbsorptionDensity0ConstantTerm = -2.0 / 3.0;
    info.AbsorptionDensity0LinearTerm = 1.0 / (15.0 * scalar);
    info.AbsorptionDensity1ConstantTerm = 8.0 / 3.0;
    info.AbsorptionDensity1LinearTerm = -1.0 / (15.0 * scalar);
    info.AbsorptionExtinction = vec3<f32>(0.000650, 0.001881, 0.000085);

    // Cloud parameters
    info.CloudBaseHeight = 2.5; // Base height of clouds in km
    info.CloudTopHeight = 10.0; // Top height of clouds in km
    info.CloudScattering = vec3<f32>(0.9, 0.9, 0.9); // Albedo of clouds
    info.CloudAbsorption = vec3<f32>(0.001, 0.001, 0.001);
    info.CloudPhaseG = 0.8;
    info.CloudK = 0.9;

    return info;
}


fn getAlbedo(scattering: vec3<f32>, extinction: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        scattering.x / max(0.001, extinction.x),
        scattering.y / max(0.001, extinction.y),
        scattering.z / max(0.001, extinction.z)
    );
}

fn sampleMediumRGB(WorldPos: vec3<f32>, Atmosphere: AtmosphereParameters) -> MediumSampleRGB {
    var viewHeight: f32 = length(WorldPos) - Atmosphere.BottomRadius;

    var densityMie: f32 = exp(Atmosphere.MieDensityExpScale * viewHeight);
    var densityRay: f32 = exp(Atmosphere.RayleighDensityExpScale * viewHeight);
    var clampVal: f32 = Atmosphere.AbsorptionDensity1LinearTerm * viewHeight + Atmosphere.AbsorptionDensity1ConstantTerm;
    if viewHeight < Atmosphere.AbsorptionDensity0LayerWidth {
        clampVal = Atmosphere.AbsorptionDensity0LinearTerm * viewHeight + Atmosphere.AbsorptionDensity0ConstantTerm;
    }
    var densityOzo: f32 = clamp(clampVal, 0.0, 1.0);

    var s: MediumSampleRGB;

    s.scatteringMie = densityMie * Atmosphere.MieScattering;
    s.absorptionMie = densityMie * Atmosphere.MieAbsorption;
    s.extinctionMie = densityMie * Atmosphere.MieExtinction;

    s.scatteringRay = densityRay * Atmosphere.RayleighScattering;
    s.absorptionRay = vec3<f32>(0.0, 0.0, 0.0);
    s.extinctionRay = s.scatteringRay + s.absorptionRay;

    s.scatteringOzo = vec3<f32>(0.0, 0.0, 0.0);
    s.absorptionOzo = densityOzo * Atmosphere.AbsorptionExtinction;
    s.extinctionOzo = s.scatteringOzo + s.absorptionOzo;

    if uniformBuffer.enable_clouds > .5 {
        var cloudDensity: f32 = sampleCloudDensity(WorldPos, Atmosphere);
        s.scatteringCloud = cloudDensity * Atmosphere.CloudScattering;
        s.absorptionCloud = cloudDensity * Atmosphere.CloudAbsorption;
        s.extinctionCloud = s.scatteringCloud + s.absorptionCloud;
    }

    s.scattering = s.scatteringMie + s.scatteringRay + s.scatteringOzo + s.scatteringCloud;
    s.absorption = s.absorptionMie + s.absorptionRay + s.absorptionOzo + s.absorptionCloud;
    s.extinction = s.extinctionMie + s.extinctionRay + s.extinctionOzo + s.extinctionCloud;
    s.albedo = getAlbedo(s.scattering, s.extinction);

    return s;
}

fn sigmoid(x: f32) -> f32 {
    return 1.0 / (1.0 + exp(-x));
}

fn sampleCloudDensity(WorldPos: vec3<f32>, Atmosphere: AtmosphereParameters) -> f32 {
    var x: f32 = length(WorldPos) - Atmosphere.BottomRadius;
    var ymid: f32 = (Atmosphere.CloudTopHeight - Atmosphere.CloudBaseHeight) / 2.0;
    var ymin: f32 = Atmosphere.CloudBaseHeight;
    var ymax: f32 = Atmosphere.CloudTopHeight;
    var baseVal: f32 = smoothstep(ymin, ymid, x) * (1 - smoothstep(ymid, ymax, x));

    // 8km cube
    var noiseScale: f32 = 1. / 8.0;
    var P = WorldPos + vec3<f32>(0.0, -Atmosphere.BottomRadius, 0.0);
    var S = P * noiseScale + vec3<f32>(0.5, 0.0, 0.5);
    var noiseValue: f32 = sampleCloudTexture(S);
    var spread = 0.1;
    var center = 1.0 - uniformBuffer.cloud_coverage + 0.0;
    var fromV = max(0.0, center - spread);
    var toV = min(1.0, center + spread);
    noiseValue = smoothstep(fromV, toV, noiseValue);
    return noiseValue;
}

// sample the cloud texture
fn sampleCloudTexture(pos: vec3<f32>) -> f32 {
    let clamped = clamp(pos, vec3(0.0), vec3(1.0));
    if (clamped.x != pos.x || clamped.y != pos.y || clamped.z != pos.z) {
        return 0.0;
    }
    return textureSampleLevel(cloudTexture, cloudTextureSampler, pos, 0.0).r;
}

fn UvToLutTransmittanceParams(Atmosphere: AtmosphereParameters, uv: vec2<f32>) -> UvToLutResult {
    var result: UvToLutResult;

    var x_mu: f32 = uv.x;
    var x_r: f32 = uv.y;

    var H: f32 = sqrt(Atmosphere.TopRadius * Atmosphere.TopRadius - Atmosphere.BottomRadius * Atmosphere.BottomRadius);
    var rho: f32 = H * x_r;
    result.viewHeight = sqrt(rho * rho + Atmosphere.BottomRadius * Atmosphere.BottomRadius);

    var d_min: f32 = Atmosphere.TopRadius - result.viewHeight;
    var d_max: f32 = rho + H;
    var d: f32 = d_min + x_mu * (d_max - d_min);
    result.viewZenithCosAngle = (H * H - rho * rho - d * d) / (2.0 * result.viewHeight * d);
    if d == 0.0 {
        result.viewZenithCosAngle = 1.0;
    }
    result.viewZenithCosAngle = clamp(result.viewZenithCosAngle, -1.0, 1.0);

    return result;
}

fn RenderTransmittanceLutPS(pixPos: vec2<f32>, uv: vec2<f32>, texSizeF32: vec2<f32>) -> vec4<f32> {
    var Atmosphere: AtmosphereParameters = GetAtmosphereParameters();
    var transmittanceParams: UvToLutResult = UvToLutTransmittanceParams(Atmosphere, uv);

    var WorldPos: vec3<f32> = vec3<f32>(0.0, 0.0, transmittanceParams.viewHeight);
    var WorldDir: vec3<f32> = vec3<f32>(0.0, sqrt(1.0 - transmittanceParams.viewZenithCosAngle * transmittanceParams.viewZenithCosAngle), transmittanceParams.viewZenithCosAngle);

    var ground = false;
    var SampleCountIni = 40.0;	// Can go a low as 10 sample but energy lost starts to be visible.
    var DepthBufferValue = -1.0;
    var VariableSampleCount = false;
    var MieRayPhase = false;

    var scatteringResult: SingleScatteringResult = IntegrateScatteredLuminance(pixPos, WorldPos, WorldDir, getSunDirection(), Atmosphere, ground, SampleCountIni, DepthBufferValue, VariableSampleCount, MieRayPhase, defaultTMaxMax, texSizeF32);
    var transmittance: vec3<f32> = exp(-scatteringResult.OpticalDepth);

    // transmittance = vec3<f32>(uv, 0.0);

    // Optical depth to transmittance
    return vec4<f32>(transmittance, 1.0);
}

var<workgroup> MultiScatAs1SharedMem: array<vec3<f32>, 64>;
var<workgroup> LSharedMem: array<vec3<f32>, 64>;

var<private> SQRTSAMPLECOUNT: u32 = 8;
var<private> MultipleScatteringFactor: f32 = 1.0; // change to 50 to see the texture

fn RenderMultipleScatteringLutPS(pixPos: vec2<f32>, texSizeF32: vec2<f32>, ThreadId: vec3<u32>, input_uv: vec2<f32>) -> vec4<f32> {
    var uv = vec2<f32>(fromSubUvsToUnit(input_uv.x, MultiScatteringLUTRes), fromSubUvsToUnit(input_uv.y, MultiScatteringLUTRes));

    var Atmosphere: AtmosphereParameters = GetAtmosphereParameters();

    var cosSunZenithAngle: f32 = uv.x * 2.0 - 1.0;
    var sunDir: vec3<f32> = vec3<f32>(0.0, sqrt(saturate(1.0 - cosSunZenithAngle * cosSunZenithAngle)), cosSunZenithAngle);
    var viewHeight: f32 = Atmosphere.BottomRadius + saturate(uv.y + PLANET_RADIUS_OFFSET) * (Atmosphere.TopRadius - Atmosphere.BottomRadius - PLANET_RADIUS_OFFSET);

    var WorldPos: vec3<f32> = vec3<f32>(0.0, 0.0, viewHeight);
    var WorldDir: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);

    const ground: bool = true;
    const SampleCountIni: f32 = 20.0;
    const DepthBufferValue: f32 = -1.0;
    const VariableSampleCount: bool = false;
    const MieRayPhase: bool = false;

    var SphereSolidAngle: f32 = 4.0 * PI;
    var IsotropicPhase: f32 = 1.0 / SphereSolidAngle;


    var sqrtSample: f32 = f32(SQRTSAMPLECOUNT);
    var i: f32 = 0.5 + f32(ThreadId.z / SQRTSAMPLECOUNT);
    var j: f32 = 0.5 + f32(ThreadId.z - u32(f32(ThreadId.z / SQRTSAMPLECOUNT) * f32(SQRTSAMPLECOUNT)));
    {
        var randA: f32 = i / sqrtSample;
        var randB: f32 = j / sqrtSample;
        var theta: f32 = 2.0 * PI * randA;
        var phi: f32 = acos(1.0 - 2.0 * randB);
        var cosPhi: f32 = cos(phi);
        var sinPhi: f32 = sin(phi);
        var cosTheta: f32 = cos(theta);
        var sinTheta: f32 = sin(theta);
        WorldDir.x = cosTheta * sinPhi;
        WorldDir.y = sinTheta * sinPhi;
        WorldDir.z = cosPhi;
        var result: SingleScatteringResult = IntegrateScatteredLuminance(pixPos, WorldPos, WorldDir, sunDir, Atmosphere, ground, SampleCountIni, DepthBufferValue, VariableSampleCount, MieRayPhase, defaultTMaxMax, texSizeF32);

        MultiScatAs1SharedMem[ThreadId.z] = result.MultiScatAsOne * SphereSolidAngle / (sqrtSample * sqrtSample);
        LSharedMem[ThreadId.z] = result.L * SphereSolidAngle / (sqrtSample * sqrtSample);
    }

    workgroupBarrier();

    if ThreadId.z < 32 {
        MultiScatAs1SharedMem[ThreadId.z] += MultiScatAs1SharedMem[ThreadId.z + 32];
        LSharedMem[ThreadId.z] += LSharedMem[ThreadId.z + 32];
    }
    workgroupBarrier();

    if ThreadId.z < 16 {
        MultiScatAs1SharedMem[ThreadId.z] += MultiScatAs1SharedMem[ThreadId.z + 16];
        LSharedMem[ThreadId.z] += LSharedMem[ThreadId.z + 16];
    }
    workgroupBarrier();

    if ThreadId.z < 8 {
        MultiScatAs1SharedMem[ThreadId.z] += MultiScatAs1SharedMem[ThreadId.z + 8];
        LSharedMem[ThreadId.z] += LSharedMem[ThreadId.z + 8];
    }
    workgroupBarrier();
    if ThreadId.z < 4 {
        MultiScatAs1SharedMem[ThreadId.z] += MultiScatAs1SharedMem[ThreadId.z + 4];
        LSharedMem[ThreadId.z] += LSharedMem[ThreadId.z + 4];
    }
    workgroupBarrier();
    if ThreadId.z < 2 {
        MultiScatAs1SharedMem[ThreadId.z] += MultiScatAs1SharedMem[ThreadId.z + 2];
        LSharedMem[ThreadId.z] += LSharedMem[ThreadId.z + 2];
    }
    workgroupBarrier();
    if ThreadId.z < 1 {
        MultiScatAs1SharedMem[ThreadId.z] += MultiScatAs1SharedMem[ThreadId.z + 1];
        LSharedMem[ThreadId.z] += LSharedMem[ThreadId.z + 1];
    }
    workgroupBarrier();
    if ThreadId.z > 0 {
        return vec4<f32>(0.0);
    }

    var MultiScatAsOne: vec3<f32> = MultiScatAs1SharedMem[0] * IsotropicPhase;
    var InScatteredLuminance: vec3<f32> = LSharedMem[0] * IsotropicPhase;

    var L: vec3<f32> = vec3<f32>(0.0);

    var r: vec3<f32> = MultiScatAsOne;
    var SumOfAllMultiScatteringEventsContribution: vec3<f32> = 1.0 / (1.0 - r);
    L = InScatteredLuminance * SumOfAllMultiScatteringEventsContribution;

    return vec4<f32>(L, 1.0);
}

fn RenderSkyPS(uv: vec2<f32>, pixPos: vec2<f32>, texSizeF32: vec2<f32>, WorldPos: vec3<f32>, WorldDir: vec3<f32>, DepthBufferValue: f32) -> SingleScatteringResult {
    var coords = vec2<i32>(uv * vec2<f32>(textureDimensions(multipleScatteringTexture, 0)));
    var sampleA = textureSampleLevel(multipleScatteringTexture, multipleScatteringTextureSampler, uv, 0.0).rgb;

    coords = vec2<i32>(uv * vec2<f32>(textureDimensions(transmittanceTexture, 0)));
    var sampleB = textureSampleLevel(transmittanceTexture, transmittanceTextureSampler, uv, 0.0).rgb;

    // sample the cloud texture
    
    var sampleC = textureSampleLevel(cloudTexture, cloudTextureSampler, vec3<f32>(0.0, 0.0, 0.0), 0.0).rgb;

    var Atmosphere: AtmosphereParameters = GetAtmosphereParameters();

    var eyePosition = uniformBuffer.eye_position;
    var SunDir = uniformBuffer.sun_position;

    const ground = false;
    const SampleCountIni = 30.0;
    const VariableSampleCount = true;
    const MieRayPhase = true;
    var result: SingleScatteringResult = IntegrateScatteredLuminance(pixPos, WorldPos, WorldDir, SunDir, Atmosphere, ground, SampleCountIni, DepthBufferValue, VariableSampleCount, MieRayPhase, defaultTMaxMax, texSizeF32);

    return result;
}

// the max distance to ray march in meters
var<private> defaultTMaxMax: f32 = 9000000.0;
var<private> PLANET_RADIUS_OFFSET: f32 = 0.01;

// Sample per pixel for ray marching
// 16.0 without clouds
// 128.0 for thicker atmosphere
// 256.0 for clouds
var<private> RayMarchMinMaxSPP: vec2<f32> = vec2<f32>(1.0, 16.0);
var<private> RayMarchMinMaxSPPCloud: vec2<f32> = vec2<f32>(1.0, 256.0);
var<private> MULTISCATAPPROX_ENABLED: u32 = 1;
var<private> VOLUMETRIC_SHADOW_ENABLED: u32 = 1;
var<private> MultiScatteringLUTRes: f32 = 32.0;

struct SingleScatteringResult {
    L: vec3<f32>,                        // Scattered light (luminance)
    OpticalDepth: vec3<f32>,             // Optical depth (1/m)
    Transmittance: vec3<f32>,            // Transmittance in [0,1] (unitless)
    MultiScatAsOne: vec3<f32>,
    NewMultiScatStep0Out: vec3<f32>,
    NewMultiScatStep1Out: vec3<f32>,
};

// Sun's surface luminance ~1.6 × 10⁹ cd/m²
var<private> SunLuminance: vec3<f32> = vec3<f32>(1.6e9);

struct AtmosphereParameters {
    BottomRadius: f32,
    TopRadius: f32,

    RayleighDensityExpScale: f32,
    RayleighScattering: vec3<f32>,

    MieDensityExpScale: f32,
    MieScattering: vec3<f32>,
    MieExtinction: vec3<f32>,
    MieAbsorption: vec3<f32>,
    MiePhaseG: f32,

    AbsorptionDensity0LayerWidth: f32,
    AbsorptionDensity0ConstantTerm: f32,
    AbsorptionDensity0LinearTerm: f32,
    AbsorptionDensity1ConstantTerm: f32,
    AbsorptionDensity1LinearTerm: f32,
    AbsorptionExtinction: vec3<f32>,

    GroundAlbedo: vec3<f32>,

    CloudBaseHeight: f32,
    CloudTopHeight: f32,
    CloudScattering: vec3<f32>,
    CloudAbsorption: vec3<f32>,
    CloudPhaseG: f32,
    CloudK: f32,
};

struct MediumSampleRGB {
    scattering: vec3<f32>,
    absorption: vec3<f32>,
    extinction: vec3<f32>,

    scatteringMie: vec3<f32>,
    absorptionMie: vec3<f32>,
    extinctionMie: vec3<f32>,

    scatteringRay: vec3<f32>,
    absorptionRay: vec3<f32>,
    extinctionRay: vec3<f32>,

    scatteringOzo: vec3<f32>,
    absorptionOzo: vec3<f32>,
    extinctionOzo: vec3<f32>,

    scatteringCloud: vec3<f32>,
    absorptionCloud: vec3<f32>,
    extinctionCloud: vec3<f32>,

    albedo: vec3<f32>,
};

fn ComputeSphereNormal(coord: vec2<f32>, phiStart: f32, phiLength: f32, thetaStart: f32, thetaLength: f32) -> vec3<f32> {
    var normal: vec3<f32>;
    normal.x = -sin(thetaStart + coord.y * thetaLength) * sin(phiStart + coord.x * phiLength);
    normal.y = -cos(thetaStart + coord.y * thetaLength);
    normal.z = -sin(thetaStart + coord.y * thetaLength) * cos(phiStart + coord.x * phiLength);
    return normalize(normal);
}

fn raySphereIntersectNearest(r0: vec3<f32>, rd: vec3<f32>, s0: vec3<f32>, sR: f32) -> f32 {
    var a: f32 = dot(rd, rd);
    var s0_r0: vec3<f32> = r0 - s0;
    var b: f32 = 2.0 * dot(rd, s0_r0);
    var c: f32 = dot(s0_r0, s0_r0) - (sR * sR);
    var delta: f32 = b * b - 4.0 * a * c;
    if delta < 0.0 || a == 0.0 {
        return -1.0;
    }
    var sol0: f32 = (-b - sqrt(delta)) / (2.0 * a);
    var sol1: f32 = (-b + sqrt(delta)) / (2.0 * a);
    if sol0 < 0.0 && sol1 < 0.0 {
        return -1.0;
    }
    if sol0 < 0.0 {
        return max(0.0, sol1);
    } else if sol1 < 0.0 {
        return max(0.0, sol0);
    }
    return max(0.0, min(sol0, sol1));
}

struct SphereIntersection {
    near: f32,
    far: f32,
}

fn raySphereIntersect(r0: vec3<f32>, rd: vec3<f32>, s0: vec3<f32>, sR: f32) -> SphereIntersection {
    var result: SphereIntersection;
    result.near = -1.0;
    result.far = -1.0;

    let a = dot(rd, rd);
    let s0_r0 = r0 - s0;
    let b = 2.0 * dot(rd, s0_r0);
    let c = dot(s0_r0, s0_r0) - (sR * sR);
    let delta = b * b - 4.0 * a * c;

    if delta < 0.0 || a == 0.0 {
        return result;
    }

    let sqrtDelta = sqrt(delta);
    let sol0 = (-b - sqrtDelta) / (2.0 * a);
    let sol1 = (-b + sqrtDelta) / (2.0 * a);

    // Check if ray origin is inside sphere
    let isInside = dot(s0_r0, s0_r0) < (sR * sR);
    
    if isInside {
        result.near = 0.0;
        result.far = max(0.0, sol1);
    } else {
        result.near = max(0.0, sol0);
        result.far = max(0.0, sol1);
    }

    return result;
}

fn CornetteShanksMiePhaseFunction(g: f32, cosTheta: f32) -> f32 {
    var k: f32 = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
    return k * (1.0 + cosTheta * cosTheta) / pow(1.0 + g * g - 2.0 * g * -cosTheta, 1.5);
}

fn RayleighPhase(cosTheta: f32) -> f32 {
    var factor: f32 = 3.0 / (16.0 * PI);
    return factor * (1.0 + cosTheta * cosTheta);
}

fn hgPhase(g: f32, cosTheta: f32) -> f32 {
    return CornetteShanksMiePhaseFunction(g, cosTheta);
}

// dual-lobe hg phase 
fn dualLobeHgPhase(g: f32, cosTheta: f32, k: f32) -> f32 {
    var phase1: f32 = hgPhase(g, cosTheta);
    var phase2: f32 = hgPhase(-g, cosTheta);
    return mix(phase1, phase2, k);
}

fn LutTransmittanceParamsToUv(Atmosphere: AtmosphereParameters, viewHeight: f32, viewZenithCosAngle: f32) -> vec2<f32> {
    var H: f32 = sqrt(max(0.0, Atmosphere.TopRadius * Atmosphere.TopRadius - Atmosphere.BottomRadius * Atmosphere.BottomRadius));
    var rho: f32 = sqrt(max(0.0, viewHeight * viewHeight - Atmosphere.BottomRadius * Atmosphere.BottomRadius));

    var discriminant: f32 = viewHeight * viewHeight * (viewZenithCosAngle * viewZenithCosAngle - 1.0) + Atmosphere.TopRadius * Atmosphere.TopRadius;
    var d: f32 = max(0.0, (-viewHeight * viewZenithCosAngle + sqrt(discriminant))); // Distance to atmosphere boundary

    var d_min: f32 = Atmosphere.TopRadius - viewHeight;
    var d_max: f32 = rho + H;
    var x_mu: f32 = (d - d_min) / (d_max - d_min);
    var x_r: f32 = rho / H;

    return vec2<f32>(x_mu, x_r);
}

fn fromUnitToSubUvs(u: f32, resolution: f32) -> f32 {
    return (u + 0.5 / resolution) * (resolution / (resolution + 1.0));
}

fn fromSubUvsToUnit(u: f32, resolution: f32) -> f32 {
    return (u - 0.5 / resolution) * (resolution / (resolution - 1.0));
}

fn GetSunLuminance(WorldPos: vec3<f32>, WorldDir: vec3<f32>, PlanetRadius: f32) -> vec3<f32> {
    var sun_direction: vec3<f32> = normalize(getSunDirection());
    if dot(WorldDir, sun_direction) > cos(0.5 * 0.505 * PI / 180.0) {
        var t: f32 = raySphereIntersectNearest(WorldPos, WorldDir, vec3<f32>(0.0, 0.0, 0.0), PlanetRadius);
        if t < 0.0 { // no intersection
            return SunLuminance; // arbitrary. But fine, not use when comparing the models
    }
    }
    return vec3<f32>(0.0);
}

fn MoveToTopAtmosphere(WorldPos: ptr<function, vec3<f32>>, WorldDir: vec3<f32>, AtmosphereTopRadius: f32) -> bool {
    var viewHeight: f32 = length(*WorldPos);
    if viewHeight > AtmosphereTopRadius {
        var tTop: f32 = raySphereIntersectNearest(*WorldPos, WorldDir, vec3<f32>(0.0, 0.0, 0.0), AtmosphereTopRadius);
        if tTop >= 0.0 {
            var UpVector: vec3<f32> = *WorldPos / viewHeight;
            var UpOffset: vec3<f32> = UpVector * -0.01;
            *WorldPos = *WorldPos + WorldDir * tTop + UpOffset;
        } else {
            // Ray is not intersecting the atmosphere
            return false;
        }
    }
    return true; // ok to start tracing
}

fn getSunDirection() -> vec3<f32> {
    return uniformBuffer.sun_position;
}

fn GetTransmittanceToSun(Atmosphere: AtmosphereParameters, P: vec3<f32>, sunDir: vec3<f32>) -> vec3<f32> {
    var pHeight: f32 = length(P);
    var UpVector: vec3<f32> = P / pHeight;
    var SunZenithCosAngle: f32 = dot(sunDir, UpVector);
    var uv = LutTransmittanceParamsToUv(Atmosphere, pHeight, SunZenithCosAngle);
    return textureSampleLevel(transmittanceTexture, transmittanceTextureSampler, uv, 0.0).rgb;
}

fn GetMultipleScattering(Atmosphere: AtmosphereParameters, scattering: vec3<f32>, extinction: vec3<f32>, worlPos: vec3<f32>, viewZenithCosAngle: f32) -> vec3<f32> {
    var uv = saturate(vec2<f32>(viewZenithCosAngle * 0.5 + 0.5, (length(worlPos) - Atmosphere.BottomRadius) / (Atmosphere.TopRadius - Atmosphere.BottomRadius)));
    uv = vec2<f32>(fromUnitToSubUvs(uv.x, MultiScatteringLUTRes), fromUnitToSubUvs(uv.y, MultiScatteringLUTRes));

    var multiScatteredLuminance: vec3<f32> = textureSampleLevel(multipleScatteringTexture, multipleScatteringTextureSampler, uv, 0.0).rgb;
    return multiScatteredLuminance * uniformBuffer.multiple_scattering_factor;
}

fn computeVolumetricShadow(WorldPos: vec3<f32>, LightDir: vec3<f32>, Atmosphere: AtmosphereParameters) -> f32 {
    var shadow: f32 = 1.0;
    var stepSize: f32 = 0.3; // Adjust based on scene scale
    var pos: vec3<f32> = WorldPos;
    for (var i: f32 = 0.0; i < 16.0; i += 1.0) { // Number of steps can be adjusted
        pos += stepSize * LightDir;
        shadow *= 1.0 - sampleCloudDensity(pos, Atmosphere);
        if (shadow < 0.05) {
            break; // Early exit for low shadow values
        }
    }
    return shadow;
}

// near: 0.01, far: 10000
fn linearizeDepth(depth: f32, near: f32, far: f32) -> f32 {
    var z: f32 = depth * 2.0 - 1.0; // Back to NDC
    return (2.0 * near * far) / (far + near - z * (far - near));
}

fn IntegrateScatteredLuminance(
    pixPos: vec2<f32>,
    StartPos: vec3<f32>,
    StartDir: vec3<f32>,
    SunDir: vec3<f32>,
    Atmosphere: AtmosphereParameters,
    ground: bool,
    SampleCountIni: f32,
    DepthBufferValue: f32,
    VariableSampleCount: bool,
    MieRayPhase: bool,
    tMaxMax: f32,
    resolution: vec2<f32>
) -> SingleScatteringResult {
    var result: SingleScatteringResult = SingleScatteringResult(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));

    var ClipSpace: vec3<f32> = vec3<f32>((pixPos / resolution) * vec2<f32>(2.0, 2.0) - vec2<f32>(1.0, 1.0), 1.0);
    // Check if camera is below atmosphere's bottom radius
    var WorldPos: vec3<f32> = StartPos;
    var WorldDir: vec3<f32> = StartDir;
    var cameraHeight = length(WorldPos);
    if cameraHeight < Atmosphere.BottomRadius {
        // Find intersection with the ground
        var tGround = raySphereIntersectNearest(WorldPos, WorldDir, vec3<f32>(0.0), Atmosphere.BottomRadius);
        WorldPos = WorldPos + WorldDir * (tGround + 0.1);
    }

    // Compute next intersection with atmosphere or ground
    var earthO: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
    var tBottom: f32 = raySphereIntersectNearest(WorldPos, WorldDir, earthO, Atmosphere.BottomRadius);
    var tTop: f32 = raySphereIntersectNearest(WorldPos, WorldDir, earthO, Atmosphere.TopRadius);
    var tMax: f32 = 0.0;
    if tBottom < 0.0 {
        if tTop < 0.0 {
            tMax = 0.0; // No intersection with earth nor atmosphere: stop right away  
            return result;
        } else {
            tMax = tTop;
        }
    } else {
        if tTop > 0.0 {
            tMax = min(tTop, tBottom);
        }
    }

    
    if DepthBufferValue >= 0.0 {
        ClipSpace.z = DepthBufferValue;
        if ClipSpace.z < 1.0 {
            #ifdef USE_DEPTH_BUFFER
                // should be using inv view projection matrix instead of just view_from_clip

                // construct the inv view projection matrix
                // var inv_proj = view.view_from_clip;
                // var inv_view = view.world_from_view;
                // var inv_view_proj = inv_view * inv_proj;

                // var DepthBufferWorldPos: vec4<f32> = view.view_from_clip * vec4<f32>(ClipSpace, 1.0);
                // DepthBufferWorldPos /= DepthBufferWorldPos.w;
                // var tDepth: f32 = length(DepthBufferWorldPos.xyz - (WorldPos + vec3<f32>(0.0, -Atmosphere.BottomRadius, 0.0))); // apply earth offset to go back to origin as top of earth mode. 
                // tMax = min(tMax, tDepth);


                // Transform directly from clip to world space using the pre-computed inverse view-projection matrix
                var DepthBufferWorldPos: vec4<f32> = view.world_from_clip * vec4<f32>(ClipSpace, 1.0);
                DepthBufferWorldPos /= DepthBufferWorldPos.w;
                var offset = WorldPos + vec3<f32>(0.0, -Atmosphere.BottomRadius, 0.0);
                var tDepth: f32 = length(DepthBufferWorldPos.xyz - offset);
                tMax = min(tMax, tDepth);
            #endif
        }
    }
    tMax = min(tMax, tMaxMax);

    // Sample count
    var SampleCount: f32 = SampleCountIni;
    var SampleCountFloor: f32 = SampleCountIni;
    var tMaxFloor: f32 = tMax;
    if VariableSampleCount {
        var spp: vec2<f32> = vec2<f32>(1.0, uniformBuffer.max_raymarch_samples);
        SampleCount = mix(spp.x, spp.y, clamp(tMax * 0.01, 0.0, 1.0));
        SampleCountFloor = floor(SampleCount);
        tMaxFloor = tMax * SampleCountFloor / SampleCount; // rescale tMax to map to the last entire step segment.
    }
    var dt: f32 = tMax / SampleCount;

    // Phase functions
    var uniformPhase: f32 = 1.0 / (4.0 * PI);
    var wi: vec3<f32> = SunDir;
    var wo: vec3<f32> = WorldDir;
    var cosTheta: f32 = dot(wi, wo);
    var MiePhaseValue: f32 = hgPhase(Atmosphere.MiePhaseG, -cosTheta); // negate cosTheta because WorldDir is an "in" direction.
    var RayleighPhaseValue: f32 = RayleighPhase(cosTheta);
    var CloudPhaseValue: f32 = dualLobeHgPhase(Atmosphere.CloudPhaseG, cosTheta, Atmosphere.CloudK);

    // #ifdef ILLUMINANCE_IS_ONE
    var globalL: vec3<f32> = vec3<f32>(1.0);
    // #else
    //   var globalL: vec3<f32> = iSunIlluminance;
    // #endif

    // Ray march the atmosphere to integrate optical depth
    var L: vec3<f32> = vec3<f32>(0.0);
    var throughput: vec3<f32> = vec3<f32>(1.0);
    var OpticalDepth: vec3<f32> = vec3<f32>(0.0);
    var t: f32 = 0.0;
    var tPrev: f32 = 0.0;
    var SampleSegmentT: f32 = 0.3;

    // TODO: improve sampling and performance inside of the cloud layer
    // compute the intersection points pointing in WorldDir direction
    var tCloudBottom: f32 = raySphereIntersectNearest(WorldPos, WorldDir, earthO, Atmosphere.BottomRadius + Atmosphere.CloudBaseHeight);
    var tCloudTop: f32 = raySphereIntersectNearest(WorldPos, WorldDir, earthO, Atmosphere.BottomRadius + Atmosphere.CloudTopHeight);

    // Ray marching loop
    for (var s: f32 = 0.0; s < SampleCount; s += 1.0) {
        if VariableSampleCount {
            var t0: f32 = s / SampleCountFloor;
            var t1: f32 = (s + 1.0) / SampleCountFloor;
            t0 = t0 * t0;
            t1 = t1 * t1;
            t0 = tMaxFloor * t0;
            if t1 > 1.0 {
                t1 = tMax;
            } else {
                t1 = tMaxFloor * t1;
            }
            t = t0 + (t1 - t0) * SampleSegmentT;
            dt = t1 - t0;
        } else {
            var NewT: f32 = tMax * (s + SampleSegmentT) / SampleCount;
            dt = NewT - t;
            t = NewT;
        }
        var P: vec3<f32> = WorldPos + t * WorldDir;

        var medium: MediumSampleRGB = sampleMediumRGB(P, Atmosphere);
        var SampleOpticalDepth: vec3<f32> = medium.extinction * dt;
        var SampleTransmittance: vec3<f32> = exp(-SampleOpticalDepth);
        OpticalDepth += SampleOpticalDepth;

        var pHeight: f32 = length(P);
        var UpVector: vec3<f32> = P / pHeight;
        var SunZenithCosAngle: f32 = dot(SunDir, UpVector);
        var uv = LutTransmittanceParamsToUv(Atmosphere, pHeight, SunZenithCosAngle);
        var transmittanceTextureSize = vec2<f32>(textureDimensions(transmittanceTexture, 0));
        var transmittanceTextureCoord = vec2<i32>(transmittanceTextureSize * uv);
        var TransmittanceToSun: vec3<f32> = textureLoad(transmittanceTexture, transmittanceTextureCoord, 0).rgb;

        var PhaseTimesScattering: vec3<f32>;
        if MieRayPhase {
            PhaseTimesScattering = medium.scatteringMie * MiePhaseValue + medium.scatteringRay * RayleighPhaseValue + medium.scatteringCloud * CloudPhaseValue;
        } else {
            PhaseTimesScattering = medium.scattering * uniformPhase;
        }

        // Earth shadow
        var tEarth: f32 = raySphereIntersectNearest(P, SunDir, earthO + PLANET_RADIUS_OFFSET * UpVector, Atmosphere.BottomRadius);
        var earthShadow: f32 = 1.0;
        if tEarth >= 0.0 {
            earthShadow = 0.0;
        }

        // Dual scattering for multi scattering
        var multiScatteredLuminance: vec3<f32> = vec3<f32>(0.0);
        if MULTISCATAPPROX_ENABLED == 1 {
            multiScatteredLuminance = GetMultipleScattering(Atmosphere, medium.scattering, medium.extinction, P, SunZenithCosAngle);
        }

        var shadow: f32 = 1.0;
        if (uniformBuffer.enable_volumetric_shadows > 0.5) {
            #ifdef USE_SHADOW_MAP
                shadow = getShadow(P, WorldDir);
            #endif
        }
        var height: f32 = length(WorldPos) - Atmosphere.BottomRadius;
        var cloudShadow: f32 = 1.0;
        if uniformBuffer.enable_clouds > 0.5 {
            cloudShadow = computeVolumetricShadow(P, SunDir, Atmosphere);
        }

        var S: vec3<f32> = globalL * (earthShadow * shadow * cloudShadow * TransmittanceToSun * PhaseTimesScattering + multiScatteredLuminance * medium.scattering);

        var MS: vec3<f32> = medium.scattering * 1.0;
        var MSint: vec3<f32> = (MS - MS * SampleTransmittance) / medium.extinction;
        result.MultiScatAsOne += throughput * MSint;

        var Sint: vec3<f32> = (S - S * SampleTransmittance) / medium.extinction;
        L += throughput * Sint;
        throughput *= SampleTransmittance;

        // Early exit if opacity is close to 1
        if all(throughput < vec3<f32>(0.001)) {
            break;
        }

        tPrev = t;
    }

    if ground && tMax == tBottom && tBottom > 0.0 {
        // Account for bounced light off the earth
        var P: vec3<f32> = WorldPos + tBottom * WorldDir;
        var pHeight: f32 = length(P);
        var UpVector: vec3<f32> = P / pHeight;
        var NdotL: f32 = clamp(dot(normalize(UpVector), normalize(SunDir)), 0.0, 1.0);
        var albedo: vec3<f32> = Atmosphere.GroundAlbedo;
        var TransmittanceToSun: vec3<f32> = GetTransmittanceToSun(Atmosphere, P, SunDir);
        L += globalL * TransmittanceToSun * throughput * NdotL * albedo / PI;
    }

    result.L = L;
    result.OpticalDepth = OpticalDepth;
    result.Transmittance = throughput;
    return result;
}
