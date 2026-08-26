# Task contract — CAUSAL-BIAS-01 binary-confounder bias-function sensitivity

Status: complete

Date: 2026-08-25

Parent requirements: Part XII §110.2 `BiasFunctionSensitivity`, CAU-01B, WS-82.

`marklab causal bias-sensitivity` consumes a finite observed additive effect and a bounded nonempty
grid of uniquely identified binary-confounder scenarios. Each scenario declares treated and control
confounder prevalence in `[0,1]` and a finite additive confounder effect on the outcome scale. The
explicit sensitivity model is `bias=(prevalence_treated-prevalence_control)*outcome_effect` and
`adjusted_effect=observed_effect-bias`.

The result retains every parameter/bias/adjusted value, whether its sign differs from a nonzero
observed effect, the min/max sensitivity region, zero inclusion, scenario/work limits, and a
model-specific claim ceiling. For observed effect two, prevalence/effect scenarios producing biases
`0.6`, `-0.6`, and `3` yield adjusted values `1.4`, `2.6`, and `-1`, hence region `[-1,2.6]` crossing
zero. This is sensitivity arithmetic under supplied assumptions, not identification or correction.
