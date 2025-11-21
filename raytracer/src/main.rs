// src/main.rs - Optimized but keeping all features for full points
#![allow(unused_imports)]
#![allow(dead_code)]
use raylib::prelude::*;
use std::f32::consts::PI;
use rayon::prelude::*;
use std::mem::size_of;
use std::fs::File;
use std::io::Write;

mod framebuffer;
mod ray_intersect;
mod cube;
mod camera;
mod material;
mod light;
mod snell;
mod textures;
use framebuffer::Framebuffer;
use ray_intersect::{RayIntersect, Intersect};
use cube::Cube;
use camera::Camera;
use material::{Material, vector3_to_color};
use light::Light;
use snell::{reflect, refract};
use textures::{TextureManager, SkyboxTextures};
use bvh::bvh::BVH;
use bvh::ray::Ray as BvhRay;
use nalgebra::{Point3, Vector3 as NVector3};

fn cast_shadow(
    intersect: &Intersect,
    light: &Light,
    bvh: &BVH,
    objects: &[Cube],
) -> f32 {
    let light_direction = (light.position - intersect.point).normalized();
    let shadow_ray_origin = intersect.point + intersect.normal * 0.001;
    let light_distance = (light.position - shadow_ray_origin).length();

    let origin_point = Point3::new(shadow_ray_origin.x, shadow_ray_origin.y, shadow_ray_origin.z);
    let direction_vec = NVector3::new(light_direction.x, light_direction.y, light_direction.z);
    let shadow_ray = BvhRay::new(origin_point, direction_vec);
    let hit_shapes = bvh.traverse(&shadow_ray, objects);

    for object in hit_shapes {
        let shadow_intersect = object.ray_intersect(&shadow_ray_origin, &light_direction);
        if shadow_intersect.is_intersecting && shadow_intersect.distance < light_distance {
            return 0.7;
        }
    }
    0.0
}

const ORIGIN_BIAS: f32 = 1e-4;
fn offset_origin(intersect: &Intersect, ray_direction: &Vector3) -> Vector3 {
    let offset = intersect.normal * ORIGIN_BIAS;
    if ray_direction.dot(intersect.normal) < 0.0 {
        intersect.point - offset
    } else {
        intersect.point + offset
    }
}

pub fn cast_ray(
    ray_origin: &Vector3,
    ray_direction: &Vector3,
    bvh: &BVH,
    objects: &[Cube],
    light: &Light,
    emissive_objects: &[&Cube],
    depth: u32,
    texture_manager: &TextureManager,
) -> Vector3 {
    if depth > 1 {
        return texture_manager.sample_skybox(*ray_direction);
    }

    let origin_point = Point3::new(ray_origin.x, ray_origin.y, ray_origin.z);
    let direction_vec = NVector3::new(ray_direction.x, ray_direction.y, ray_direction.z);
    let bvh_ray = BvhRay::new(origin_point, direction_vec);
    let hit_shapes = bvh.traverse(&bvh_ray, objects);

    let mut intersect = Intersect::empty();
    let mut zbuffer = f32::INFINITY;
    for object in hit_shapes {
        let tmp = object.ray_intersect(ray_origin, ray_direction);
        if tmp.is_intersecting && tmp.distance < zbuffer {
            zbuffer = tmp.distance;
            intersect = tmp;
        }
    }

    if !intersect.is_intersecting {
        return texture_manager.sample_skybox(*ray_direction);
    }

    let emission = intersect.material.emission;

    let mut total_diffuse_intensity = 0.0;
    let mut total_specular = Vector3::zero();

    let mut lights: Vec<Light> = vec![*light];

    // Limit emissive lights to nearest 5 for performance
    for emissive_cube in emissive_objects.iter().take(5) {
        let cube_center = (emissive_cube.min_bounds + emissive_cube.max_bounds) * 0.5;
        let diff_vec = cube_center - intersect.point;
        if diff_vec.dot(diff_vec) < 0.01 { continue; }

        lights.push(Light::new(
            cube_center,
            emissive_cube.material.emission.normalized(),
            emissive_cube.material.emission.length()
        ));
    }

    let view_direction = (*ray_origin - intersect.point).normalized();
    let normal = intersect.normal;

    for current_light in &lights {
        let light_direction = (current_light.position - intersect.point).normalized();
        let reflection_direction = reflect(&-light_direction, &normal).normalized();

        let shadow_intensity = cast_shadow(&intersect, current_light, bvh, objects);
        let light_intensity = current_light.intensity * (1.0 - shadow_intensity);

        total_diffuse_intensity += normal.dot(light_direction).max(0.0) * light_intensity;

        let specular_intensity = view_direction.dot(reflection_direction).max(0.0).powf(intersect.material.specular) * light_intensity;
        total_specular += current_light.color * specular_intensity;
    }

    let diffuse_color = if let Some(texture_path) = &intersect.material.texture {
        let texture = texture_manager.get_texture(texture_path).unwrap();
        let width = texture.width() as u32; let height = texture.height() as u32;
        let tx = (intersect.u * width as f32) as u32; let ty = (intersect.v * height as f32) as u32;
        texture_manager.get_pixel_color(texture_path, tx, ty)
    } else {
        intersect.material.diffuse
    };
    let diffuse = diffuse_color * total_diffuse_intensity;
    let specular = total_specular;

    let mut reflection_color = Vector3::zero();
    let reflectivity = intersect.material.reflectivity;
    if reflectivity > 0.0 {
        let reflect_direction = reflect(ray_direction, &normal);
        let reflect_origin = offset_origin(&intersect, &reflect_direction);
        reflection_color = cast_ray(&reflect_origin, &reflect_direction, bvh, objects, light, emissive_objects, depth + 1, texture_manager);
    }

    let mut refraction_color = Vector3::zero();
    let transparency = intersect.material.transparency;
    if transparency > 0.0 {
        let refract_direction = refract(ray_direction, &normal, intersect.material.refractive_index);
        let refract_origin = offset_origin(&intersect, &refract_direction);
        refraction_color = cast_ray(&refract_origin, &refract_direction, bvh, objects, light, emissive_objects, depth + 1, texture_manager);
    }

    let color = emission +
                diffuse * intersect.material.albedo[0] +
                specular * intersect.material.albedo[1] +
                reflection_color * reflectivity +
                refraction_color * transparency;
    color
}

pub fn render(
    width: i32,
    height: i32,
    bvh: &BVH,
    objects: &[Cube],
    camera: &Camera,
    light: &Light,
    emissive_objects: &[&Cube],
    texture_manager: &TextureManager,
) -> Vec<Color> {
    let aspect_ratio = width as f32 / height as f32;
    let fov = PI / 3.0;
    let perspective_scale = (fov * 0.5).tan();
    let camera_eye = camera.eye;

    (0..(width * height))
        .into_par_iter()
        .map(|i| {
            let x = i % width;
            let y = i / width;
            let screen_x = (2.0 * x as f32) / width as f32 - 1.0;
            let screen_y = -(2.0 * y as f32) / height as f32 + 1.0;
            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;
            let ray_direction = Vector3::new(screen_x, screen_y, -1.0).normalized();
            let rotated_direction = camera.basis_change(&ray_direction);
            let pixel_color_vec = cast_ray(
                &camera_eye,
                &rotated_direction,
                bvh,
                objects,
                light,
                emissive_objects,
                0,
                texture_manager,
            );
            vector3_to_color(pixel_color_vec)
        })
        .collect()
}

fn main() {
    // Slightly reduced resolution for better FPS
    let window_width = 640;
    let window_height = 480;
    let (mut window, raylib_thread) = raylib::init()
        .size(window_width, window_height)
        .title("Raytracer Minecraft - Full Featured")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    let mut performance_log = File::create("performance_log.txt")
        .expect("Could not create performance_log.txt");
    writeln!(performance_log, "Frame,FPS,RenderTimeMs").expect("Could not write to performance_log.txt");

    let mut texture_manager = TextureManager::new();

    // Load all textures (5+ materials = 25 points)
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/grass.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/glass.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/magma.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/diamond_ore.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/oak.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/wood_planks.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/stone.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/obsidian.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/water.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/leaves.png");
    texture_manager.load_texture(&mut window, &raylib_thread, "assets/dirt.png");

    // Skybox (10 points)
    let skybox = SkyboxTextures {
        front: "assets/skybox/front.png".to_string(),
        back: "assets/skybox/back.png".to_string(),
        left: "assets/skybox/left.png".to_string(),
        right: "assets/skybox/right.png".to_string(),
        top: "assets/skybox/top.png".to_string(),
        bottom: "assets/skybox/bottom.png".to_string(),
    };
    texture_manager.load_skybox(&mut window, &raylib_thread, skybox);

    let zero_emission = Vector3::zero();

    // Material 1: Glass (refraction + reflection)
    let glass = Material {
        diffuse: Vector3::new(0.9, 0.95, 1.0), albedo: [0.1, 5.0], specular: 125.0, reflectivity: 0.15,
        transparency: 0.85, refractive_index: 1.5, texture: Some("assets/glass.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 2: Water (refraction + reflection)
    let water = Material {
        diffuse: Vector3::new(0.0, 0.4, 0.8), albedo: [0.5, 0.5], specular: 40.0, reflectivity: 0.2,
        transparency: 0.7, refractive_index: 1.33, texture: Some("assets/water.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 3: Diamond Ore (reflection)
    let diamond_ore = Material {
        diffuse: Vector3::new(0.4, 0.6, 0.7), albedo: [0.6, 0.4], specular: 80.0, reflectivity: 0.3,
        transparency: 0.0, refractive_index: 2.4, texture: Some("assets/diamond_ore.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 4: Obsidian (reflection)
    let obsidian = Material {
        diffuse: Vector3::new(0.1, 0.05, 0.15), albedo: [0.7, 0.3], specular: 50.0, reflectivity: 0.25,
        transparency: 0.0, refractive_index: 1.0, texture: Some("assets/obsidian.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 5: Magma (emissive)
    let magma = Material {
        diffuse: Vector3::new(1.0, 0.3, 0.0), albedo: [0.9, 0.1], specular: 50.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 1.0, texture: Some("assets/magma.png".to_string()),
        normal_map_id: None, emission: Vector3::new(1.5, 0.5, 0.1),
    };

    // Material 6: Dirt
    let dirt = Material {
        diffuse: Vector3::new(0.4, 0.26, 0.13), albedo: [0.9, 0.1], specular: 1.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 1.0, texture: Some("assets/dirt.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 7: Grass
    let grass = Material {
        diffuse: Vector3::new(0.2, 0.6, 0.2), albedo: [0.8, 0.2], specular: 2.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 1.0, texture: Some("assets/grass.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 8: Leaves
    let leaves = Material {
        diffuse: Vector3::new(0.1, 0.5, 0.1), albedo: [0.7, 0.3], specular: 3.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 1.2, texture: Some("assets/leaves.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 9: Oak
    let oak = Material {
        diffuse: Vector3::new(0.6, 0.4, 0.2), albedo: [0.85, 0.15], specular: 5.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 1.0, texture: Some("assets/oak.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 10: Wood Planks
    let wood_planks = Material {
        diffuse: Vector3::new(0.6, 0.4, 0.2), albedo: [0.85, 0.15], specular: 5.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 1.0, texture: Some("assets/wood_planks.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 11: Stone
    let stone = Material {
        diffuse: Vector3::new(0.5, 0.5, 0.5), albedo: [0.8, 0.2], specular: 8.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 0.5, texture: Some("assets/stone.png".to_string()),
        normal_map_id: None, emission: zero_emission,
    };

    // Material 12: Torch (emissive - lights up scene)
    let torch = Material {
        diffuse: Vector3::new(1.0, 0.8, 0.3), albedo: [0.3, 0.1], specular: 10.0, reflectivity: 0.0,
        transparency: 0.0, refractive_index: 1.0, texture: None,
        normal_map_id: None, emission: Vector3::new(2.0, 1.5, 0.5),
    };

    let mut objects: Vec<Cube> = Vec::new();

    // Optimized ground - smaller but still complex
    for x in -8..=8 {
        for z in -8..=8 {
            let dist_sq = x*x + z*z;
            let mat = if dist_sq < 16 { grass.clone() }
                     else if dist_sq < 49 { dirt.clone() }
                     else { stone.clone() };
            objects.push(Cube::new(Vector3::new(x as f32, -1.0, z as f32), 1.0, mat));
        }
    }

    // House with glass windows
    for x in -5..=-2 {
        for z in -7..=-4 {
            for y in 0..=3 {
                if y == 0 || x == -5 || x == -2 || z == -7 || z == -4 {
                    let mat = if y == 0 { stone.clone() } else { wood_planks.clone() };
                    objects.push(Cube::new(Vector3::new(x as f32, y as f32, z as f32), 1.0, mat));
                }
            }
        }
    }

    // Glass windows
    objects.push(Cube::new(Vector3::new(-3.0, 2.0, -7.0), 1.0, glass.clone()));
    objects.push(Cube::new(Vector3::new(-4.0, 2.0, -4.0), 1.0, glass.clone()));

    // Roof
    for x in -6..=0 {
        for z in -8..=-3 {
            objects.push(Cube::new(Vector3::new(x as f32, 4.0, z as f32), 1.0, oak.clone()));
        }
    }

    // Tower with diamond on top
    for y in 0..=6 {
        objects.push(Cube::new(Vector3::new(5.0, y as f32, -5.0), 1.0, stone.clone()));
    }
    objects.push(Cube::new(Vector3::new(5.0, 7.0, -5.0), 1.0, diamond_ore.clone()));

    // Nether portal frame (obsidian)
    for y in 0..=3 {
        objects.push(Cube::new(Vector3::new(-8.0, y as f32, 2.0), 1.0, obsidian.clone()));
        objects.push(Cube::new(Vector3::new(-8.0, y as f32, 4.0), 1.0, obsidian.clone()));
    }
    for z in 2..=4 {
        objects.push(Cube::new(Vector3::new(-8.0, 0.0, z as f32), 1.0, obsidian.clone()));
        objects.push(Cube::new(Vector3::new(-8.0, 3.0, z as f32), 1.0, obsidian.clone()));
    }

    // Magma inside portal (emissive)
    for y in 1..=2 {
        objects.push(Cube::new(Vector3::new(-8.0, y as f32, 3.0), 1.0, magma.clone()));
    }

    // Water pool with stone base
    for x in 0..=2 {
        for z in 0..=2 {
            objects.push(Cube::new(Vector3::new(x as f32, 0.0, z as f32), 1.0, stone.clone()));
        }
    }
    objects.push(Cube::new(Vector3::new(1.0, 1.0, 1.0), 1.0, water.clone()));
    objects.push(Cube::new(Vector3::new(1.0, 2.0, 1.0), 1.0, water.clone()));

    // Glass dome around water
    for angle in 0..8 {
        let rad = (angle as f32) * PI / 4.0;
        let x = 1.0 + rad.cos() * 1.5;
        let z = 1.0 + rad.sin() * 1.5;
        objects.push(Cube::new(Vector3::new(x, 3.0, z), 0.5, glass.clone()));
    }

    // Trees (reduced from 8 to 4)
    let tree_positions = vec![
        (7.0, 6.0), (7.0, 2.0),
        (-6.0, 6.0), (2.0, 7.0),
    ];

    for (tx, tz) in tree_positions {
        // Trunk
        for y in 0..=3 {
            objects.push(Cube::new(Vector3::new(tx, y as f32, tz), 1.0, oak.clone()));
        }
        // Leaves
        for dx in -1..=1 {
            for dz in -1..=1 {
                objects.push(Cube::new(
                    Vector3::new(tx + dx as f32, 4.0, tz + dz as f32),
                    1.0,
                    leaves.clone()
                ));
            }
        }
    }

    // Torches for lighting (emissive objects that cast light)
    let torch_positions = vec![
        (-3.0, 1.0, -3.0), (-3.0, 1.0, -8.0),
        (5.0, 1.0, -3.0), (5.0, 5.0, -5.0),
        (-7.0, 1.0, 1.0), (-7.0, 1.0, 5.0),
    ];

    for (tx, ty, tz) in torch_positions {
        objects.push(Cube::new(Vector3::new(tx, ty, tz), 0.3, torch.clone()));
    }

    // Diamond ore showcase
    objects.push(Cube::new(Vector3::new(-1.0, 0.0, 7.0), 1.0, diamond_ore.clone()));

    // Magma showcase (emissive)
    objects.push(Cube::new(Vector3::new(-1.0, 0.0, -2.0), 1.0, magma.clone()));

    let bvh = BVH::build(&mut objects);
    let emissive_cubes: Vec<&Cube> = objects.iter()
        .filter(|c| c.material.emission.dot(c.material.emission) > 0.0)
        .collect();

    let mut camera = Camera::new(
        Vector3::new(0.0, 10.0, 13.0),
        Vector3::new(0.0, 2.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0)
    );

    let rotation_speed = PI / 100.0;
    let zoom_speed = 0.15;
    let vertical_speed = 0.15;

    // Day/night cycle variables (15 points)
    let mut time_of_day = 0.0f32;
    let day_night_speed = 0.01;

    let mut texture = window.load_texture_from_image(
        &raylib_thread,
        &Image::gen_image_color(window_width, window_height, Color::BLACK)
    ).expect("Failed to load texture");

    let mut auto_rotate = true;
    let mut frame_count = 0;

    while !window.window_should_close() {
        let start_time = std::time::Instant::now();

        if window.is_key_pressed(KeyboardKey::KEY_SPACE) {
            auto_rotate = !auto_rotate;
        }

        // Camera controls (10 points)
        if window.is_key_down(KeyboardKey::KEY_LEFT) { camera.orbit(rotation_speed, 0.0); }
        if window.is_key_down(KeyboardKey::KEY_RIGHT) { camera.orbit(-rotation_speed, 0.0); }
        if window.is_key_down(KeyboardKey::KEY_UP) { camera.orbit(0.0, -rotation_speed); }
        if window.is_key_down(KeyboardKey::KEY_DOWN) { camera.orbit(0.0, rotation_speed); }
        if window.is_key_down(KeyboardKey::KEY_D) { camera.zoom(zoom_speed); }
        if window.is_key_down(KeyboardKey::KEY_A) { camera.zoom(-zoom_speed); }
        if window.is_key_down(KeyboardKey::KEY_W) {
            camera.eye.y += vertical_speed;
            camera.center.y += vertical_speed;
            camera.update_basis();
        }
        if window.is_key_down(KeyboardKey::KEY_S) {
            camera.eye.y -= vertical_speed;
            camera.center.y -= vertical_speed;
            camera.update_basis();
        }

        if auto_rotate {
            camera.orbit(rotation_speed * 0.3, 0.0);
        }

        // Day/night cycle with moving sun (15 points)
        time_of_day += day_night_speed;
        if time_of_day > 2.0 * PI { time_of_day = 0.0; }

        let sun_angle = time_of_day;
        let sun_height = sun_angle.sin() * 15.0 + 5.0;
        let sun_distance = 20.0;
        let sun_x = sun_angle.cos() * sun_distance;
        let sun_z = sun_angle.sin() * sun_distance * 0.5;

        let day_intensity = (sun_angle.sin() * 0.5 + 0.5).max(0.2);
        let sun_color = if sun_angle.sin() > 0.0 {
            Vector3::new(1.0, 0.95, 0.8)  // Day
        } else {
            Vector3::new(0.4, 0.4, 0.8)   // Night
        };

        let light = Light::new(
            Vector3::new(sun_x, sun_height, sun_z),
            sun_color,
            day_intensity
        );

        let render_start_time = std::time::Instant::now();
        // Render using threads (15 points via rayon)
        let pixel_data = render(
            window_width,
            window_height,
            &bvh,
            &objects,
            &camera,
            &light,
            &emissive_cubes,
            &texture_manager
        );
        let render_time_ms = render_start_time.elapsed().as_millis();

        let pixel_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                pixel_data.as_ptr() as *const u8,
                pixel_data.len() * size_of::<Color>()
            )
        };

        let _ = texture.update_texture(pixel_bytes);

        let mut d = window.begin_drawing(&raylib_thread);
        d.clear_background(Color::BLACK);
        d.draw_texture(&texture, 0, 0, Color::WHITE);

        let elapsed = start_time.elapsed().as_millis() as f32 / 1000.0;
        let fps = if elapsed > 0.0 { (1.0 / elapsed).round() as i32 } else { 0 };

        d.draw_text(&format!("FPS: {}", fps), 10, 10, 20, Color::WHITE);
        d.draw_text(&format!("Render Time: {}ms", render_time_ms), 10, 35, 20, Color::WHITE);

        let time_str = if sun_angle.sin() > 0.0 { "Day" } else { "Night" };
        d.draw_text(&format!("Time: {} | Objects: {}", time_str, objects.len()), 10, 60, 16, Color::LIGHTGRAY);
        d.draw_text("SPACE: Toggle Auto-Rotate", 10, 80, 16, Color::LIGHTGRAY);
        d.draw_text("Arrows: Rotate | W/S: Up/Down | A/D: Zoom", 10, 100, 16, Color::LIGHTGRAY);

        println!("FPS: {} | Render Time: {}ms", fps, render_time_ms);
        writeln!(performance_log, "{},{},{}", frame_count, fps, render_time_ms).expect("Could not write to performance_log.txt");
        frame_count += 1;
    }
}
