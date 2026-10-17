# Device studio concept

Standalone, code-led UI exploration; no application or daemon integration.

```sh
python3 -m http.server 34056 --bind 127.0.0.1 --directory prototypes/device-studio
```

Open http://127.0.0.1:34056 . English is the default; Russian is selectable.

Flows: device overview; capability-specific navigation; lighting color, brightness and effect selection; cooling presets; LCD theme selection and simple layer visibility/label editing; separate per-device draft, apply and revert state. State lasts only for the page session. All devices, readings and curves are illustrative. SL-INFINITY is a future-family design example, not a claim of current support. Effect timing and manual curve editing are not simulated.

No installation, USB access, remote assets or production application changes.
