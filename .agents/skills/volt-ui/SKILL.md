---
name: volt-ui
description: Use Volt UI as the default source for Angular UI atoms, component markup, selectors, theming, and CLI/MCP lookup in Lattice.
---

# Volt UI

Use this skill whenever you add, edit, review, or debug Angular UI in Lattice.

Volt UI is the default UI atom source for this project. Do not hand-roll buttons, inputs, cards, badges, form fields, switches, tabs, tables, dialogs, popovers, tooltips, menus, skeletons, separators, progress, or similar UI atoms when Volt UI has a matching component or primitive. Compose product UI from Volt UI atoms first, then add only the local layout, state, copy, and feature behavior Lattice needs.

## Decision Rules

- Prefer copied Volt UI components with the consumer naming convention: `ui-*` selectors and `UiXxx` class names.
- Import copied components from the local UI folder/barrel, not directly from `@voltui/components`, unless the existing code is explicitly using the package workflow.
- Before inventing a new atom, check whether Volt UI already provides it. Use the MCP server when unsure.
- Only create a bespoke UI primitive when Volt UI lacks the atom, the product requirement truly cannot be expressed by composition, or an existing local component is already the project-owned abstraction.
- Keep bespoke pieces thin and local: feature layout, composition, responsive behavior, and app-specific state are fine; replacing a Volt UI atom with custom HTML/CSS is not.
- Use semantic Tailwind tokens such as `bg-primary`, `text-foreground`, `border-border`, `bg-muted`, `rounded-md`; avoid hard-coded CSS variables or one-off color systems.
- Follow Lattice Angular rules too: standalone components, signals, zoneless-friendly state, and no NgModules.

## Source Of Truth

Volt UI exposes a Streamable HTTP MCP server at:

```text
https://volt-ui.pages.dev/api/mcp
```

Use it for current component APIs instead of guessing. The important tools are:

- `list_components` for the component catalog.
- `get_component` for selectors, inputs, outputs, subcomponents, and examples.
- `get_usage_example` for import paths and snippets.
- `get_theme_info` for theme provider, color presets, style presets, and dark mode.
- `get_project_info` for architecture and naming conventions.
- `generate_cli_command` for correct `@voltui/cli` commands.

## CLI Workflow

Volt UI's recommended consumer workflow is source ownership:

```bash
npx @voltui/cli init
npx @voltui/cli add button card form-field input
```

For this Bun workspace, prefer the Bun equivalent when actually running commands:

```bash
bunx @voltui/cli init
bunx @voltui/cli add button card form-field input
```

Use `--dry-run` before adding or overwriting UI atoms when the destination is unclear.

## Common Atoms

- Actions: `button`, `toggle`, `toggle-group`, `toolbar`, `dropdown-menu`.
- Form atoms: `input`, `textarea`, `checkbox`, `radio`, `switch`, `select`, `slider`, `range-slider`, `form-field`, `search`, `combobox`, `date-picker`, `file-upload`, `input-otp`, `listbox`.
- Surfaces and display: `card`, `badge`, `avatar`, `separator`, `skeleton`, `progress`, `meter`, `table`.
- Navigation/layout: `tabs`, `accordion`, `breadcrumbs`, `pagination`, `navigation-menu`, `sidebar`, `resizable`.
- Overlays: `dialog`, `drawer`, `popover`, `tooltip`, `toast`.
- Theme utilities: `theme`, `provideVoltTheme`, `applyVoltTheme`.

## Selector Patterns

Element selectors are for presentational components:

```html
<ui-button>Save</ui-button>
<ui-input [formControl]="email" type="email" />
<ui-card>
  <ui-card-header>
    <ui-card-title>Settings</ui-card-title>
  </ui-card-header>
  <ui-card-content>...</ui-card-content>
</ui-card>
```

Attribute directives are used when behavior attaches to an existing host:

```html
<button [uiDialog]="dialogTpl">Open</button>
<button uiTooltip [uiTooltip]="tooltipTpl">Help</button>
<img uiAvatarImage src="/avatar.png" alt="Profile" />
```

## Overlay Rules

Overlays are template-based. Do not write fake trigger elements such as `<ui-dialog>`, `<ui-tooltip>`, `<ui-popover-trigger>`, or `<ui-dropdown-menu-trigger>`.

```html
<button [uiDialog]="dialogTpl">Open</button>

<ng-template #dialogTpl let-close="close">
  <div uiDialogOverlay></div>
  <div uiDialogContent>
    <h2 uiDialogTitle>Confirm</h2>
    <p uiDialogDescription>Are you sure?</p>
    <ui-button (click)="close()">Confirm</ui-button>
  </div>
</ng-template>
```

Use the same trigger plus `ng-template` shape for drawer, popover, tooltip, and dropdown menu.

## Forms

Use Reactive Forms for CVA-backed controls and import `ReactiveFormsModule` where needed.

```ts
import { FormControl, ReactiveFormsModule } from '@angular/forms';
import { UiCheckbox, UiInput, UiSwitch } from './ui';
```

```html
<ui-form-field>
  <ui-form-field-label>Email</ui-form-field-label>
  <ui-input [formControl]="email" type="email" />
  <ui-form-field-hint>Used for account updates.</ui-form-field-hint>
</ui-form-field>

<ui-checkbox [formControl]="accepted">Accept terms</ui-checkbox>
<ui-switch [formControl]="enabled">Enabled</ui-switch>
```

## Theme Setup

If the project is not already themed, import the Volt theme CSS globally and provide the theme at bootstrap:

```css
@import 'tailwindcss';
@import '@voltui/components/themes.css';
```

```ts
import { provideVoltTheme } from '@voltui/components';

providers: [provideVoltTheme({ color: 'volt', style: 'sharp', dark: false })];
```

Color presets: `volt`, `ember`, `sage`, `dusk`, `glacier`.
Style presets: `sharp`, `soft`, `brutal`, `ghost`, `retro`.

## Guardrails

- Do not invent inputs or outputs. Query the MCP or inspect copied source first.
- Keep Volt UI atoms accessible by preserving their selectors, host directives, CVA wiring, and keyboard/focus behavior.
- Do not replace Volt UI internals with local CSS unless the user explicitly asks for a customized copied component.
- When reviewing Angular UI, flag newly hand-rolled atoms that should be Volt UI components.
