use marklab_prediction::{
    area_under_roc, average_precision, cross_validate_logistic, ClassWeight, LogisticCvSpec,
    LogisticError, LogisticProblem,
};

fn table(rows: usize, columns: usize, value: impl Fn(usize, usize) -> f64) -> Vec<f64> {
    (0..rows)
        .flat_map(|row| (0..columns).map(move |column| (row, column)))
        .map(|(row, column)| value(row, column))
        .collect()
}

fn assert_probabilities(problem: &LogisticProblem, c: f64, test: &[f64], expected: &[f64]) {
    let model = problem.fit(c).unwrap();
    let columns = test.len() / expected.len();
    for (row, expected) in test.chunks_exact(columns).zip(expected) {
        let actual = model.probability(row).unwrap();
        assert!(
            (actual - expected).abs() < 1e-7,
            "probability {actual} differs from scikit-learn's {expected}"
        );
    }
}

// Expected probabilities from scikit-learn 1.9.0: StandardScaler() followed by
// LogisticRegression(C, class_weight, solver="newton-cholesky", tol=1e-14) on the same tables.
// Its lbfgs solver agrees within 1e-8.
#[test]
fn l2_logistic_regression_matches_scikit_learn() {
    // More features than rows (solved in the row space), balanced class weights, and a constant
    // column whose test value differs from training.
    let features = table(10, 14, |i, j| {
        if j == 5 {
            2.5
        } else {
            (1.3 * i as f64 + 0.7 * j as f64).sin() + 0.1 * j as f64 * (i % 3) as f64
        }
    });
    let labels = [1, 0, 0, 1, 0, 1, 0, 0, 1, 0].map(|y| y == 1);
    let problem = LogisticProblem::new(&features, 14, &labels, ClassWeight::Balanced).unwrap();
    let test = table(3, 14, |t, j| {
        if j == 5 {
            7.0
        } else {
            (0.9 * t as f64 + 0.4 * j as f64).cos()
        }
    });
    assert_probabilities(
        &problem,
        0.5,
        &test,
        &[0.3981220065771219, 0.3457388245362915, 0.33208059021143255],
    );

    // More rows than features, unweighted.
    let features = table(30, 4, |i, j| {
        (0.37 * i as f64 * (j + 1) as f64).sin() + 0.02 * i as f64 * j as f64
    });
    let labels = (0..30)
        .map(|i| (2.1 * i as f64).sin() + 0.8 * features[i * 4] > 0.0)
        .collect::<Vec<_>>();
    let problem = LogisticProblem::new(&features, 4, &labels, ClassWeight::Uniform).unwrap();
    let test = table(3, 4, |t, j| (0.5 * t as f64 + j as f64).cos());
    assert_probabilities(
        &problem,
        2.0,
        &test,
        &[0.6534872332138117, 0.5903672852930246, 0.48766732694537784],
    );
}

#[test]
fn ranking_metrics_match_scikit_learn_with_tied_scores() {
    let labels = [1, 0, 1, 1, 0, 0, 1, 0].map(|y| y == 1);
    let scores = [0.9, 0.8, 0.8, 0.4, 0.4, 0.2, 0.1, 0.1];
    // roc_auc_score and average_precision_score in scikit-learn 1.9.0.
    assert_eq!(area_under_roc(&labels, &scores), Some(0.65625));
    let precision = average_precision(&labels, &scores).unwrap();
    assert!((precision - 0.6916666666666667).abs() < 1e-15);
    assert_eq!(area_under_roc(&[true, true], &[0.1, 0.2]), None);
}

fn grouped_data() -> (Vec<f64>, Vec<bool>, Vec<usize>) {
    // 15 groups of two rows; a group's rows share a label and lie close together.
    let rows = 30;
    let groups: Vec<usize> = (0..rows).map(|row| row / 2).collect();
    let labels: Vec<bool> = groups.iter().map(|group| group % 3 == 0).collect();
    let features = table(rows, 6, |row, column| {
        let signal = if labels[row] { 0.8 } else { -0.8 };
        let group = groups[row] as f64;
        signal * f64::from(u8::from(column < 2))
            + (1.7 * group + column as f64).sin()
            + 0.05 * (row % 2) as f64
    });
    (features, labels, groups)
}

fn spec(seed: u64) -> LogisticCvSpec {
    LogisticCvSpec {
        c_values: vec![0.01, 0.1, 1.0, 10.0],
        class_weight: ClassWeight::Balanced,
        folds: 3,
        inner_folds: 2,
        seed,
    }
}

#[test]
fn cross_validation_keeps_groups_together_and_repeats_from_its_seed() {
    let (features, labels, groups) = grouped_data();
    let result = cross_validate_logistic(&features, 6, &labels, &groups, &spec(7)).unwrap();
    assert_eq!(result.predictions.len(), labels.len());
    for (row, prediction) in result.predictions.iter().enumerate() {
        assert_eq!(prediction.row, row);
        let partner = row ^ 1;
        assert_eq!(
            prediction.fold, result.predictions[partner].fold,
            "group split"
        );
    }
    for fold in &result.folds {
        assert!(fold.test_positives >= 2 && fold.test_positives < fold.test_rows);
        assert_eq!(fold.training_rows + fold.test_rows, labels.len());
        assert_eq!(fold.inner_mean_auroc.len(), 4);
    }
    assert!(
        result.pooled_auroc > 0.9,
        "separable signal: {}",
        result.pooled_auroc
    );
    assert_eq!(
        cross_validate_logistic(&features, 6, &labels, &groups, &spec(7)).unwrap(),
        result
    );
    let reshuffled = cross_validate_logistic(&features, 6, &labels, &groups, &spec(8)).unwrap();
    assert!(reshuffled
        .predictions
        .iter()
        .zip(&result.predictions)
        .any(|(a, b)| a.fold != b.fold));
}

#[test]
fn cross_validation_rejects_unusable_settings() {
    let (features, labels, groups) = grouped_data();
    let cases: [(&str, LogisticCvSpec); 3] = [
        (
            "strictly increasing",
            LogisticCvSpec {
                c_values: vec![1.0, 1.0],
                ..spec(1)
            },
        ),
        (
            "folds must be between 2 and 20",
            LogisticCvSpec {
                folds: 1,
                ..spec(1)
            },
        ),
        (
            "5 groups are positive, fewer than the 6 folds",
            LogisticCvSpec {
                folds: 6,
                ..spec(1)
            },
        ),
    ];
    for (message, spec) in cases {
        match cross_validate_logistic(&features, 6, &labels, &groups, &spec) {
            Err(LogisticError::Invalid(actual)) => {
                assert!(actual.contains(message), "{actual:?} lacks {message:?}")
            }
            other => panic!("expected rejection containing {message:?}, got {other:?}"),
        }
    }
}
