---
name: Plugin Submission
about: Submit a new plugin to the Life Engine Plugin Registry
labels: plugin-submission
---

## Plugin Submission

### Plugin Details

- **Plugin ID:** `com.your-org.plugin-name`
- **Name:**
- **Version:**
- **Category:** (productivity / communication / utilities / finance / health / developer-tools / integrations / other)
- **Repository URL:**

### Checklist

- [ ] Plugin ID uses reverse-domain format (e.g. `com.example.my-plugin`)
- [ ] Version follows semver (e.g. `1.0.0`)
- [ ] `entry` field points to a valid `.js` file
- [ ] `element` field is a valid custom element name (contains hyphen)
- [ ] All declared capabilities are from the known capabilities list
- [ ] `category` is one of the valid categories
- [ ] `repository` is a public, accessible URL
- [ ] Bundle size is under 2MB (200KB recommended)
- [ ] Plugin has been tested against the current shell version
- [ ] Only the `registry/plugin-registry.json` file is modified
- [ ] Entry is inserted in alphabetical order by `id`

### Registry Entry

```json
{
  "id": "com.your-org.plugin-name",
  "name": "Plugin Name",
  "version": "1.0.0",
  "description": "Brief description of what the plugin does",
  "author": { "name": "Your Name" },
  "license": "MIT",
  "entry": "index.js",
  "element": "plugin-element",
  "minShellVersion": "0.1.0",
  "capabilities": ["ui:toast"],
  "category": "productivity",
  "repository": "https://github.com/your-org/your-plugin"
}
```

### Testing

Describe how you tested your plugin:

