use serde_json::{json, Value};

fn clamp_f64(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo { lo } else if x > hi { hi } else { x }
}

fn clamp_i32(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo { lo } else if x > hi { hi } else { x }
}

/// Build a standard MCP payload for the `call_api` tool (GIMP 3).
fn call_api_exec(python_lines: Vec<String>) -> Value {
    json!({
        "name": "call_api",
        "arguments": {
            "api_path": "exec",
            "args": ["pyGObject-console", python_lines],
            "kwargs": {}
        }
    })
}

pub fn draw_line(x1: i32, y1: i32, x2: i32, y2: i32) -> Value {
    let python_lines = vec![
        // Import GIMP 3 API
        "from gi.repository import Gimp, Gegl".to_string(),

        // Get image + first layer
        "image = Gimp.get_images()[0]".to_string(),
        "layer = image.get_layers()[0]".to_string(),

        // Set foreground color to red
        "red = Gegl.Color.new('red')".to_string(),
        "Gimp.context_set_foreground(red)".to_string(),

        // Draw a line (pencil) and flush
        format!("Gimp.pencil(layer, [{}, {}, {}, {}])", x1, y1, x2, y2),
        "Gimp.displays_flush()".to_string(),
    ];

    // json!({
    //     "name": "call_api",
    //     "arguments": {
    //         "api_path": "exec",
    //         "args": ["pyGObject-console", python_lines],
    //         "kwargs": {}
    //     }
    // })
    call_api_exec(python_lines)

}


pub fn crop_to_square() -> Value {
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        "size = min(w, h)".to_string(),
        "x = (w - size) // 2".to_string(),
        "y = (h - size) // 2".to_string(),
        "image.crop(size, size, x, y)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];

    // json!({
    //     "name": "call_api",
    //     "arguments": {
    //         "api_path": "exec",
    //         "args": ["pyGObject-console", python_lines],
    //         "kwargs": {}
    //     }
    // })
    call_api_exec(python_lines)

}

pub fn resize_width(width: i32) -> Value {
    let target_width = clamp_i32(width, 16, 8192);
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        format!("new_w = int({})", width),
        "ratio = float(new_w) / float(w)".to_string(),
        "new_h = int(h * ratio)".to_string(),
        "image.scale(new_w, new_h)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];

    // json!({
    //     "name": "call_api",
    //     "arguments": {
    //         "api_path": "exec",
    //         "args": ["pyGObject-console", python_lines],
    //         "kwargs": {}
    //     }
    // })
    call_api_exec(python_lines)
}

pub fn brightness_contrast(brightness: f64, contrast: f64) -> Value {
    let brightness = clamp_f64(brightness, -100.0, 100.0);
    let contrast = clamp_f64(contrast, -100.0, 100.0);
    let python_lines = vec![
        "from gi.repository import Gimp, Gegl".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "layer = image.get_layers()[0]".to_string(),

        // Create GEGL operation
        "op = Gegl.Node()".to_string(),
        "op.set_property('operation', 'gegl:brightness-contrast')".to_string(),

        // GEGL expects values roughly in [-1.0, 1.0]
        format!("op.set_property('brightness', {})", brightness / 100.0),
        format!("op.set_property('contrast', {})", contrast / 100.0),

        // Apply operation to the layer
        "Gimp.Drawable.apply_operation(layer, op)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];

    // json!({
    //     "name": "call_api",
    //     "arguments": {
    //         "api_path": "exec",
    //         "args": ["pyGObject-console", python_lines],
    //         "kwargs": {}
    //     }
    // })
    call_api_exec(python_lines)
}

pub fn blur(radius: f64) -> Value {
    let python_lines = vec![
        "from gi.repository import Gimp, Gegl".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "layer = image.get_layers()[0]".to_string(),

        "op = Gegl.Node()".to_string(),
        "op.set_property('operation', 'gegl:gaussian-blur')".to_string(),
        format!("op.set_property('std-dev-x', {})", radius),
        format!("op.set_property('std-dev-y', {})", radius),

        "Gimp.Drawable.apply_operation(layer, op)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];

    json!({
        "name": "call_api",
        "arguments": {
            "api_path": "exec",
            "args": ["pyGObject-console", python_lines],
            "kwargs": {}
        }
    })
}
