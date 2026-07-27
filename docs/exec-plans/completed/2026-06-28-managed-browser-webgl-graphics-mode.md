# Managed Browser WebGL Graphics Mode

Status: completed

## Summary

Lantern managed browser containers previously launched Chrome with
`--disable-gpu`, which made WebGL-oriented frontend verification unreliable for
canvas-heavy apps such as Geometis. This work adds an explicit browser start
graphics mode while preserving the old default.

## Contract

- Default managed browser behavior remains `--graphics disabled`.
- `--graphics swiftshader` removes `--disable-gpu` and launches Chrome with
  SwiftShader/ANGLE flags suitable for software WebGL smoke checks.
- `--graphics gpu` removes `--disable-gpu` without adding software renderer
  flags, for operator-managed GPU passthrough environments.
- The mode is passed to the container through `CHROME_GRAPHICS`.

## Validation

- CLI parser tests cover `--graphics swiftshader` and invalid graphics values.
- Runtime command tests cover `CHROME_GRAPHICS=disabled` and
  `CHROME_GRAPHICS=swiftshader`.
- A Geometis managed-browser WebGL smoke should use:

```sh
lantern browser start --graphics swiftshader --json
```

Then verify the viewer with `lantern flow`, `lantern console`, and a screenshot.

Validated against Geometis with a rebuilt managed browser image and:

```sh
lantern browser start --graphics swiftshader --json
```

The Geometis viewer rendered a nonblank WebGL canvas. A CDP canvas probe
reported `webgl2: true` and ANGLE over `SwiftShader Device`.
