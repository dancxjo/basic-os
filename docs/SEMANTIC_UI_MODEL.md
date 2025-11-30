# Semantic UI Model

This document defines the schema for the semantic UI model in ThingOS. The goal is to represent the UI as a graph of semantic widgets, separating the model from the view (rendering). This approach enables accessibility, testing, and alternative renderers (e.g., text-mode, screen readers) by design.

## Philosophy

1.  **Graph as Reality**: The graph is the single source of truth for the UI state. Windows, widgets, and scroll positions are nodes in the graph.
2.  **Widgets are Semantic**: Widgets are defined by their role and behavior, not their visual appearance. A "button" is a button because it acts like one, regardless of how it's drawn.
3.  **Cursor as State**: Scrollbars do not own scroll state. A `Cursor` Thing represents the scroll position over a content area. Scrollbars and other input methods manipulate this shared cursor.
4.  **Accessibility by Default**: The widget tree *is* the accessibility tree. Properties like `label`, `role`, and `description` are first-class citizens.

## Schema

### :Widget

A `Widget` represents a UI element.

**Kind**: `widget` (or a specific symbol if we decide to split them, but `widget` with a `role` property is flexible).

**Properties**:

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `role` | String | Yes | The semantic role of the widget. E.g., "window", "button", "label", "textbox", "scroll_container", "scrollbar", "list". |
| `visible` | Bool | Yes | Whether the widget is currently visible. |
| `enabled` | Bool | Yes | Whether the widget is interactive. |
| `label` | String | No | Short accessible name or caption. |
| `description` | String | No | Longer help text or description. |
| `focusable` | Bool | No | Whether the widget can receive keyboard focus. |
| `tab_index` | i64 | No | Order for keyboard navigation. |

**Relationships**:

*   `(:Widget)-[:CHILD]->(:Widget)`: Hierarchical containment. The order of `CHILD` edges defines the visual and reading order.
*   `(:Widget {role:"label"})-[:LABEL_FOR]->(:Widget)`: Semantic link between a label and the widget it describes.

### :Cursor

A `Cursor` represents a position within a scrollable content area.

**Kind**: `cursor`

**Properties**:

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `kind` | String | Yes | "vertical" or "horizontal". |
| `position` | f64 | Yes | The current scroll position. We use absolute logical units (e.g., pixels or lines) to avoid precision issues with large content, but normalized (0.0-1.0) is also an option. *Decision: Absolute logical units.* |
| `page_size` | f64 | Yes | The size of the visible viewport (in logical units). |
| `total_size` | f64 | Yes | The total size of the scrollable content (in logical units). |

**Relationships**:

*   `(:Widget {role:"scroll_container"})-[:HAS_CURSOR]->(:Cursor)`: A scroll container owns a cursor.
*   `(:Widget {role:"scroll_container"})-[:HAS_CONTENT]->(:Widget)`: Connects the container to the root of the scrolled content.
*   `(:Widget {role:"scrollbar"})-[:CONTROLS]->(:Cursor)`: A scrollbar manipulates a cursor but does not own the state.

## Example Graph

```cypher
// Window
(w:Widget {
  role: "window",
  label: "Advent Notes",
  visible: true,
  enabled: true
})

// Scroll container inside the window
(sc:Widget {
  role: "scroll_container",
  visible: true,
  enabled: true
})
(w)-[:CHILD]->(sc)

// Content root (e.g., a vertical list of items)
(content:Widget {
  role: "list"
})
(sc)-[:HAS_CONTENT]->(content)

// Cursor for vertical scrolling
(c:Cursor {
  kind: "vertical",
  position: 0.0,
  page_size: 100.0,
  total_size: 1000.0
})
(sc)-[:HAS_CURSOR]->(c)

// Scrollbar controlling the cursor
(sb:Widget {
  role: "scrollbar",
  orientation: "vertical", // specific prop for scrollbar role
  visible: true,
  enabled: true,
  focusable: true
})
(w)-[:CHILD]->(sb)
(sb)-[:CONTROLS]->(c)
```

## Future Steps

1.  **Layout Engine**: Implement a layout system that reads the `:Widget` tree and calculates positions/sizes.
2.  **Compositor Integration**: Update the compositor to render based on the `:Widget` tree.
3.  **Input Mapping**: Map keyboard and mouse events to cursor movements and widget interactions.
