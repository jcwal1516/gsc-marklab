//! Private pipe transport for the SciPy worker's pair accumulation. PCA, PCG64 and max-t
//! reduction remain in the worker; only unit codes, coordinates, projected values and row permutations
//! cross this boundary. Dimensions are checked before allocating, and batches are bounded.

use std::{
    collections::BTreeMap,
    io::{self, BufReader, BufWriter, Read, Write},
};

const PERMUTATION_BATCH: usize = 32;

pub(crate) fn run() -> io::Result<()> {
    accumulate(
        &mut BufReader::new(io::stdin().lock()),
        &mut BufWriter::new(io::stdout().lock()),
    )
}

fn accumulate(input: &mut impl Read, output: &mut impl Write) -> io::Result<()> {
    let rows = u32::from_le_bytes(read(input)?) as usize;
    let components = u32::from_le_bytes(read(input)?) as usize;
    let bins = u32::from_le_bytes(read(input)?) as usize;
    let permutations = u32::from_le_bytes(read(input)?) as usize;
    if !(2..=10_000).contains(&rows)
        || !(1..=16).contains(&components)
        || !(1..=256).contains(&bins)
        || !(20..=10_000).contains(&permutations)
    {
        return Err(invalid("invalid projected variogram dimensions"));
    }
    let mut unit_codes = Vec::with_capacity(rows);
    let mut units = BTreeMap::<u16, Vec<usize>>::new();
    for row in 0..rows {
        let code = u16::from_le_bytes(read(input)?);
        if usize::from(code) >= rows {
            return Err(invalid(
                "projected variogram biological-unit code is out of bounds",
            ));
        }
        unit_codes.push(code);
        units.entry(code).or_default().push(row);
    }
    let pairs = units
        .values()
        .map(|indices| {
            let count = indices.len() as u64;
            count * (count - 1) / 2
        })
        .sum::<u64>();
    if pairs * components as u64 * (permutations as u64 + 1) > 250_000_000 {
        return Err(invalid(
            "projected variogram work exceeds 250000000 component-pair visits",
        ));
    }
    let edges = read_floats(input, bins + 1)?;
    if edges[0] < 0.0 || edges.windows(2).any(|edge| edge[0] >= edge[1]) {
        return Err(invalid("invalid projected variogram distance bins"));
    }
    let coordinates = read_floats(input, rows * 2)?;
    let values = read_floats(input, rows * components)?;
    let mut grouped = vec![Vec::<[u16; 2]>::new(); bins];
    for indices in units.values() {
        for (position, &left) in indices.iter().enumerate() {
            for &right in &indices[position + 1..] {
                let distance = (coordinates[2 * left] - coordinates[2 * right])
                    .hypot(coordinates[2 * left + 1] - coordinates[2 * right + 1]);
                if (edges[0]..=edges[bins]).contains(&distance) {
                    // All bins are half open except the last, matching Rust result validation.
                    let bin = edges[1..]
                        .partition_point(|upper| *upper <= distance)
                        .min(bins - 1);
                    grouped[bin].push([left as u16, right as u16]);
                }
            }
        }
    }
    for group in &grouped {
        output.write_all(&(group.len() as u64).to_le_bytes())?;
    }
    write_semivariograms(output, &values, components, &grouped)?;
    output.flush()?;

    let mut permuted = vec![0.0; values.len()];
    for start in (0..permutations).step_by(PERMUTATION_BATCH) {
        let count = (permutations - start).min(PERMUTATION_BATCH);
        // Read a complete batch before writing it, so neither pipe can deadlock on capacity.
        let mut indices = vec![0_u8; count * rows * 2];
        input.read_exact(&mut indices)?;
        for permutation in indices.chunks_exact(rows * 2) {
            for (destination, index) in permutation.as_chunks::<2>().0.iter().enumerate() {
                let source = usize::from(u16::from_le_bytes(*index));
                if source >= rows {
                    return Err(invalid(
                        "projected variogram permutation index is out of bounds",
                    ));
                }
                if unit_codes[source] != unit_codes[destination] {
                    return Err(invalid(
                        "projected variogram permutation crosses biological units",
                    ));
                }
                permuted[destination * components..(destination + 1) * components]
                    .copy_from_slice(&values[source * components..(source + 1) * components]);
            }
            write_semivariograms(output, &permuted, components, &grouped)?;
            // Detect the worker closing its pipe (including its outer timeout) before
            // starting another permutation, even when this batch still has indices.
            output.flush()?;
        }
    }
    Ok(())
}

fn write_semivariograms(
    output: &mut impl Write,
    values: &[f64],
    components: usize,
    grouped: &[Vec<[u16; 2]>],
) -> io::Result<()> {
    let mut partials = Vec::with_capacity(32);
    for component in 0..components {
        for group in grouped {
            partials.clear();
            for &[left, right] in group {
                let difference = values[usize::from(left) * components + component]
                    - values[usize::from(right) * components + component];
                // Match NumPy's square followed by multiplication, including overflow policy.
                let contribution = 0.5 * (difference * difference);
                add_partial(&mut partials, contribution)?;
            }
            let semivariance = if group.is_empty() {
                f64::NAN
            } else {
                rounded_sum(&partials) / group.len() as f64
            };
            output.write_all(&semivariance.to_le_bytes())?;
        }
    }
    Ok(())
}

// Error-free partial sums with a final half-even correction preserve math.fsum's accuracy
// and permutation ties. This is the partial-sum algorithm described in CPython's mathmodule.c;
// a single compensated accumulator can differ by one ulp on halfway cases.
fn add_partial(partials: &mut Vec<f64>, mut value: f64) -> io::Result<()> {
    let mut retained = 0;
    for index in 0..partials.len() {
        let mut other = partials[index];
        if value.abs() < other.abs() {
            std::mem::swap(&mut value, &mut other);
        }
        let high = value + other;
        let low = other - (high - value);
        if low != 0.0 {
            partials[retained] = low;
            retained += 1;
        }
        value = high;
    }
    if !value.is_finite() {
        return Err(invalid("non-finite projected semivariance sum"));
    }
    partials.truncate(retained);
    if value != 0.0 {
        partials.push(value);
    }
    Ok(())
}

fn rounded_sum(partials: &[f64]) -> f64 {
    let mut remaining = partials.iter().rev();
    let mut total = *remaining.next().unwrap_or(&0.0);
    while let Some(&value) = remaining.next() {
        let high = total + value;
        let low = value - (high - total);
        total = high;
        if low != 0.0 {
            if remaining
                .next()
                .is_some_and(|next| low.is_sign_negative() == next.is_sign_negative())
            {
                let twice_low = low * 2.0;
                let rounded = total + twice_low;
                if rounded - total == twice_low {
                    total = rounded;
                }
            }
            break;
        }
    }
    total
}

fn read<const N: usize>(input: &mut impl Read) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn read_floats(input: &mut impl Read, count: usize) -> io::Result<Vec<f64>> {
    (0..count)
        .map(|_| {
            let value = f64::from_le_bytes(read(input)?);
            if value.is_finite() {
                Ok(value)
            } else {
                Err(invalid("non-finite projected variogram input"))
            }
        })
        .collect()
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_or_truncated_transport_and_excessive_work() {
        let mut header = [10_000_u32, 16, 256, 10_000]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        header.extend((0..10_000).flat_map(|_| 0_u16.to_le_bytes()));
        let error = accumulate(&mut header.as_slice(), &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("component-pair visits"));
        let error = accumulate(&mut &header[..3], &mut Vec::new()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);

        for (edges, values, units, index, message) in [
            ([1.0, 0.0], [0.0, 2.0], [0_u16, 0], 0_u16, "distance bins"),
            ([0.0, 1.0], [0.0, f64::NAN], [0, 0], 0, "non-finite"),
            ([0.0, 1.0], [0.0, 1e308], [0, 0], 0, "non-finite"),
            ([0.0, 1.0], [0.0, 2.0], [0, 0], 2, "out of bounds"),
            (
                [0.0, 1.0],
                [0.0, 2.0],
                [0, 2],
                0,
                "unit code is out of bounds",
            ),
            (
                [0.0, 1.0],
                [0.0, 2.0],
                [0, 1],
                0,
                "crosses biological units",
            ),
        ] {
            let mut bytes = [2_u32, 1, 1, 20]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            bytes.extend(units.into_iter().flat_map(u16::to_le_bytes));
            bytes.extend(
                edges
                    .into_iter()
                    .chain([0.0, 0.0, 1.0, 0.0])
                    .chain(values)
                    .flat_map(f64::to_le_bytes),
            );
            bytes.extend((0..40).flat_map(|_| index.to_le_bytes()));
            let error = accumulate(&mut bytes.as_slice(), &mut Vec::new()).unwrap_err();
            assert!(error.to_string().contains(message), "{error}");
        }
    }
}
