use serde::{Deserialize, Serialize};

/// Classic (Floor) pivot points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassicPivots {
    pub pivot: f64,
    pub r1: f64,
    pub r2: f64,
    pub r3: f64,
    pub s1: f64,
    pub s2: f64,
    pub s3: f64,
}

/// Fibonacci pivot points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FibonacciPivots {
    pub pivot: f64,
    pub r1: f64,
    pub r2: f64,
    pub r3: f64,
    pub s1: f64,
    pub s2: f64,
    pub s3: f64,
}

/// Camarilla pivot points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamarillaPivots {
    pub r1: f64,
    pub r2: f64,
    pub r3: f64,
    pub r4: f64,
    pub s1: f64,
    pub s2: f64,
    pub s3: f64,
    pub s4: f64,
}

/// Woodie pivot points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WoodiePivots {
    pub pivot: f64,
    pub r1: f64,
    pub r2: f64,
    pub s1: f64,
    pub s2: f64,
}

/// DeMark pivot points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemarkPivots {
    pub pivot: f64,
    pub r1: f64,
    pub s1: f64,
}

/// All pivot points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotPoints {
    pub classic: ClassicPivots,
    pub fibonacci: FibonacciPivots,
    pub camarilla: CamarillaPivots,
    pub woodie: WoodiePivots,
    pub demark: DemarkPivots,
}

/// Calculate Classic (Floor) pivot points
/// Formulas:
/// P = (H + L + C) / 3
/// R1 = 2P - L
/// S1 = 2P - H
/// R2 = P + Range
/// S2 = P - Range
/// R3 = H + 2(P - L)
/// S3 = L - 2(H - P)
pub fn calculate_classic(high: f64, low: f64, close: f64) -> ClassicPivots {
    let pivot = (high + low + close) / 3.0;
    let range = high - low;

    let r1 = 2.0 * pivot - low;
    let s1 = 2.0 * pivot - high;
    let r2 = pivot + range;
    let s2 = pivot - range;
    let r3 = high + 2.0 * (pivot - low);
    let s3 = low - 2.0 * (high - pivot);

    ClassicPivots {
        pivot,
        r1,
        r2,
        r3,
        s1,
        s2,
        s3,
    }
}

/// Calculate Fibonacci pivot points
/// Formulas:
/// P = (H + L + C) / 3
/// R1 = P + 0.382 * Range
/// R2 = P + 0.618 * Range
/// R3 = P + 1.000 * Range
/// S1 = P - 0.382 * Range
/// S2 = P - 0.618 * Range
/// S3 = P - 1.000 * Range
pub fn calculate_fibonacci(high: f64, low: f64, close: f64) -> FibonacciPivots {
    let pivot = (high + low + close) / 3.0;
    let range = high - low;

    let r1 = pivot + 0.382 * range;
    let r2 = pivot + 0.618 * range;
    let r3 = pivot + 1.000 * range;
    let s1 = pivot - 0.382 * range;
    let s2 = pivot - 0.618 * range;
    let s3 = pivot - 1.000 * range;

    FibonacciPivots {
        pivot,
        r1,
        r2,
        r3,
        s1,
        s2,
        s3,
    }
}

/// Calculate Camarilla pivot points
/// Formulas (using 1.1 factor variant):
/// R1 = C + 1.1 * Range / 12
/// R2 = C + 1.1 * Range / 6
/// R3 = C + 1.1 * Range / 4
/// R4 = C + 1.1 * Range / 2
/// S1 = C - 1.1 * Range / 12
/// S2 = C - 1.1 * Range / 6
/// S3 = C - 1.1 * Range / 4
/// S4 = C - 1.1 * Range / 2
pub fn calculate_camarilla(high: f64, low: f64, close: f64) -> CamarillaPivots {
    let range = high - low;
    let factor = 1.1;

    let r1 = close + factor * range / 12.0;
    let r2 = close + factor * range / 6.0;
    let r3 = close + factor * range / 4.0;
    let r4 = close + factor * range / 2.0;

    let s1 = close - factor * range / 12.0;
    let s2 = close - factor * range / 6.0;
    let s3 = close - factor * range / 4.0;
    let s4 = close - factor * range / 2.0;

    CamarillaPivots {
        r1,
        r2,
        r3,
        r4,
        s1,
        s2,
        s3,
        s4,
    }
}

/// Calculate Woodie pivot points
/// Formulas:
/// P = (H + L + 2C) / 4
/// R1 = 2P - L
/// S1 = 2P - H
/// R2 = P + Range
/// S2 = P - Range
pub fn calculate_woodie(high: f64, low: f64, close: f64) -> WoodiePivots {
    let pivot = (high + low + 2.0 * close) / 4.0;
    let range = high - low;

    let r1 = 2.0 * pivot - low;
    let s1 = 2.0 * pivot - high;
    let r2 = pivot + range;
    let s2 = pivot - range;

    WoodiePivots {
        pivot,
        r1,
        r2,
        s1,
        s2,
    }
}

/// Calculate DeMark pivot points
/// Formulas:
/// X = H + 2L + C (if C < O)
/// X = 2H + L + C (if C > O)
/// X = H + L + 2C (if C == O)
/// P = X / 4
/// R1 = X / 2 - L
/// S1 = X / 2 - H
pub fn calculate_demark(high: f64, low: f64, close: f64, open: f64) -> DemarkPivots {
    let x = if close < open {
        high + 2.0 * low + close
    } else if close > open {
        2.0 * high + low + close
    } else {
        high + low + 2.0 * close
    };

    let pivot = x / 4.0;
    let r1 = x / 2.0 - low;
    let s1 = x / 2.0 - high;

    DemarkPivots {
        pivot,
        r1,
        s1,
    }
}

/// Calculate all pivot points from OHLC data
pub fn calculate_all_pivots(open: f64, high: f64, low: f64, close: f64) -> PivotPoints {
    PivotPoints {
        classic: calculate_classic(high, low, close),
        fibonacci: calculate_fibonacci(high, low, close),
        camarilla: calculate_camarilla(high, low, close),
        woodie: calculate_woodie(high, low, close),
        demark: calculate_demark(high, low, close, open),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classic_pivots() {
        let high = 110.0;
        let low = 100.0;
        let close = 105.0;

        let pivots = calculate_classic(high, low, close);

        assert!((pivots.pivot - 105.0).abs() < 0.01);
        assert!((pivots.r1 - 110.0).abs() < 0.01);
        assert!((pivots.s1 - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_fibonacci_pivots() {
        let high = 110.0;
        let low = 100.0;
        let close = 105.0;

        let pivots = calculate_fibonacci(high, low, close);

        assert!((pivots.pivot - 105.0).abs() < 0.01);
        assert!((pivots.r3 - 115.0).abs() < 0.01); // P + Range
        assert!((pivots.s3 - 95.0).abs() < 0.01);  // P - Range
    }

    #[test]
    fn test_demark_pivots() {
        // Test case where close > open
        let pivots = calculate_demark(110.0, 100.0, 108.0, 102.0);
        assert!(pivots.pivot > 0.0);
        assert!(pivots.r1 > pivots.pivot);
        assert!(pivots.s1 < pivots.pivot);
    }
}
