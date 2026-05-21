#[derive(Clone, Copy)]
pub enum InputFormat {
    SC16Q11,
    SC16Q11M,
    F32,
    U8,
}

pub fn convert_to_magnitude(input: &[u8], format: InputFormat, output: &mut [u16]) -> usize {
    match format {
        InputFormat::SC16Q11 => convert_sc16q11(input, output),
        InputFormat::SC16Q11M => convert_sc16q11m(input, output),
        InputFormat::F32 => convert_f32(input, output),
        InputFormat::U8 => convert_u8(input, output),
    }
}

fn convert_sc16q11(input: &[u8], output: &mut [u16]) -> usize {
    let count = (input.len() / 4).min(output.len());
    for i in 0..count {
        let idx = i * 4;
        let i_val = i16::from_le_bytes([input[idx], input[idx + 1]]);
        let q_val = i16::from_le_bytes([input[idx + 2], input[idx + 3]]);
        output[i] = ((i_val as f32).powi(2) + (q_val as f32).powi(2)).sqrt() as u16;
    }
    count
}

fn convert_sc16q11m(input: &[u8], output: &mut [u16]) -> usize {
    if input.len() < 12 {
        return 0;
    }
    convert_sc16q11(&input[12..], output)
}

fn convert_f32(input: &[u8], output: &mut [u16]) -> usize {
    let count = (input.len() / 8).min(output.len());
    for i in 0..count {
        let idx = i * 8;
        let i_val = f32::from_le_bytes([
            input[idx],
            input[idx + 1],
            input[idx + 2],
            input[idx + 3],
        ]);
        let q_val = f32::from_le_bytes([
            input[idx + 4],
            input[idx + 5],
            input[idx + 6],
            input[idx + 7],
        ]);
        output[i] = ((i_val.powi(2) + q_val.powi(2)).sqrt() * 32767.0) as u16;
    }
    count
}

fn convert_u8(input: &[u8], output: &mut [u16]) -> usize {
    let count = (input.len() / 2).min(output.len());
    for i in 0..count {
        let idx = i * 2;
        let i_val = input[idx] as f32 - 128.0;
        let q_val = input[idx + 1] as f32 - 128.0;
        output[i] = ((i_val.powi(2) + q_val.powi(2)).sqrt() * 256.0) as u16;
    }
    count
}
