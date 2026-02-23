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
    let mut python_lines = vec![
        "from gi.repository import Gimp, Gegl".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "img_width = image.get_width()".to_string(),
        "img_height = image.get_height()".to_string(),
        "layer = image.get_layers()[0]".to_string(),
        "drawable = layer".to_string(),
        "layer.add_alpha() if not layer.has_alpha() else None".to_string(),
        "black = Gegl.Color.new('black')".to_string(),
        "Gimp.context_set_foreground(black)".to_string(),
    ];
    
    // We need to generate points but we don't know image size yet
    // Solution: Create a Python one-liner that does everything
    python_lines.push("exec('x1,y1,x2,y2=0,0,img_width-1,img_height-1;dx,dy=abs(x2-x1),abs(y2-y1);steps=max(dx,dy);[image.select_rectangle(Gimp.ChannelOps.ADD,int(x1+i*(x2-x1)/steps),int(y1+i*(y2-y1)/steps),5,5) for i in range(steps+1)]')".to_string());
    
    python_lines.push("drawable.edit_fill(Gimp.FillType.FOREGROUND)".to_string());
    python_lines.push("Gimp.Selection.none(image)".to_string());
    python_lines.push("Gimp.displays_flush()".to_string());

    call_api_exec(python_lines)
}

pub fn crop_to_square() -> Value {
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        "size = min(w, h)".to_string(),
        "x_offset = (w - size) // 2".to_string(),
        "y_offset = (h - size) // 2".to_string(),
        
        // FIXED: Correct GIMP 3 parameter order (new_width, new_height, offx, offy)
        "image.crop(size, size, x_offset, y_offset)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];

    call_api_exec(python_lines)
}

pub fn resize_width(width: i32) -> Value {
    let target_width = clamp_i32(width, 16, 8192);
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "w = image.get_width()".to_string(),
        "h = image.get_height()".to_string(),
        format!("new_w = int({})", target_width),
        "ratio = float(new_w) / float(w)".to_string(),
        "new_h = int(h * ratio)".to_string(),
        "image.scale(new_w, new_h)".to_string(),
        "Gimp.displays_flush()".to_string(),
    ];

    call_api_exec(python_lines)
}

pub fn brightness_contrast(brightness: f64, contrast: f64) -> Value {
    let brightness = clamp_f64(brightness, -100.0, 100.0);
    let contrast = clamp_f64(contrast, -100.0, 100.0);
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "layer = image.get_layers()[0]".to_string(),
        "drawable = layer".to_string(),

        // Use GIMP 3 brightness-contrast
        format!("Gimp.get_pdb().run_procedure('gimp-brightness-contrast', [Gimp.RunMode.NONINTERACTIVE, drawable, {}, {}])", brightness / 100.0, contrast / 100.0),
        "Gimp.displays_flush()".to_string(),
    ];

    call_api_exec(python_lines)
}

pub fn blur(radius: f64) -> Value {
    let python_lines = vec![
        "from gi.repository import Gimp".to_string(),
        "image = Gimp.get_images()[0]".to_string(),
        "layer = image.get_layers()[0]".to_string(),
        "drawable = layer".to_string(),

        // Use plug-in-gauss for blur
        format!("Gimp.get_pdb().run_procedure('plug-in-gauss', [Gimp.RunMode.NONINTERACTIVE, image, drawable, {}, {}, 0])", radius, radius),
        "Gimp.displays_flush()".to_string(),
    ];

    call_api_exec(python_lines)
}