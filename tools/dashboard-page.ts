// The dashboard page's script: draws the tree dashboard.ts left in the page with IntUI's web renderer.
import { Tree } from "../../intui/packages/core/src/index";
import { render } from "../../intui/packages/web/src/index";

const parsed = Tree.parse((globalThis as { KOKUMI_TREE?: unknown }).KOKUMI_TREE, { author: "code" });
document.body.append(
  parsed.kind === "parsed"
    ? render(parsed.value, { source: "kokumi-dashboard" })
    : Object.assign(document.createElement("pre"), { textContent: `The dashboard couldn't be drawn:\n${parsed.message}` }),
);
