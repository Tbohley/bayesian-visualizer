const SOURCE_URL = "./index.html";
const HEADER_START = "<!-- PAGE-EDITOR:HEADER:START -->";
const HEADER_END = "<!-- PAGE-EDITOR:HEADER:END -->";
const ARTICLE_START = "<!-- PAGE-EDITOR:ARTICLE:START -->";
const ARTICLE_END = "<!-- PAGE-EDITOR:ARTICLE:END -->";

const statusElement = document.querySelector("#editor-status");
let sourceHtml = "";
let editor;

function setStatus(message, isError = false) {
  statusElement.textContent = message;
  statusElement.style.color = isError ? "#ff8d8d" : "";
}

function between(source, start, end) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end);
  if (startIndex === -1 || endIndex === -1 || endIndex <= startIndex) {
    throw new Error(`Missing page editor markers: ${start} / ${end}`);
  }
  return source.slice(startIndex + start.length, endIndex).trim();
}

function replaceBetween(source, start, end, replacement) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end);
  if (startIndex === -1 || endIndex === -1 || endIndex <= startIndex) {
    throw new Error(`Missing page editor markers: ${start} / ${end}`);
  }
  return `${source.slice(0, startIndex + start.length)}\n${replacement}\n    ${source.slice(endIndex)}`;
}

function indentMarkup(markup, spaces) {
  const prefix = " ".repeat(spaces);
  return markup
    .trim()
    .split("\n")
    .map((line) => `${prefix}${line.trimEnd()}`)
    .join("\n");
}

function parseSource(source) {
  const documentNode = new DOMParser().parseFromString(source, "text/html");
  const styleElement = documentNode.querySelector("head style");
  if (!styleElement) {
    throw new Error("The source page does not contain an inline <style> block.");
  }
  return {
    header: between(source, HEADER_START, HEADER_END),
    article: between(source, ARTICLE_START, ARTICLE_END),
    styles: styleElement.textContent,
  };
}

function buildEditorComponents({ header, article }) {
  return `${header}
    <section data-bevy-placeholder="true" class="visualizer-editor-placeholder">
      <div>
        <strong>Bevy viewport</strong>
        <span>Locked in the page editor</span>
      </div>
    </section>
    ${article}`;
}

function initializeEditor(source) {
  const parsed = parseSource(source);
  const placeholderCss = `
    .visualizer-editor-placeholder {
      box-sizing: border-box;
      display: grid;
      place-items: center;
      width: 100%;
      height: 28rem;
      border-block: 1px dashed #647184;
      background: repeating-linear-gradient(135deg, #1c2028, #1c2028 12px, #20252f 12px, #20252f 24px);
      color: #d7dde7;
      font-family: Inter, system-ui, sans-serif;
      text-align: center;
    }
    .visualizer-editor-placeholder div { display: grid; gap: .4rem; }
    .visualizer-editor-placeholder span { color: #9ca8ba; font-size: .82rem; }
  `;

  editor = grapesjs.init({
    container: "#gjs",
    height: "100%",
    width: "auto",
    fromElement: false,
    storageManager: false,
    noticeOnUnload: true,
    components: buildEditorComponents(parsed),
    style: `${parsed.styles}\n${placeholderCss}`,
    blockManager: { appendTo: "#blocks" },
    styleManager: {
      appendTo: "#styles",
      sectors: [
        {
          name: "Layout",
          open: true,
          buildProps: [
            "display",
            "position",
            "width",
            "height",
            "min-width",
            "max-width",
            "margin",
            "padding",
            "gap",
            "flex-direction",
            "flex-wrap",
            "justify-content",
            "align-items",
            "overflow",
          ],
        },
        {
          name: "Typography",
          open: true,
          buildProps: [
            "font-family",
            "font-size",
            "font-weight",
            "letter-spacing",
            "line-height",
            "text-align",
            "color",
          ],
        },
        {
          name: "Appearance",
          open: false,
          buildProps: [
            "background-color",
            "border",
            "border-radius",
            "box-shadow",
            "opacity",
            "object-fit",
          ],
        },
      ],
    },
    layerManager: { appendTo: "#layers" },
    traitManager: { appendTo: "#traits" },
    selectorManager: { componentFirst: true },
    deviceManager: {
      devices: [
        { id: "Desktop", name: "Desktop", width: "" },
        { id: "Tablet", name: "Tablet", width: "768px", widthMedia: "900px" },
        { id: "Mobile", name: "Mobile", width: "390px", widthMedia: "600px" },
      ],
    },
    panels: { defaults: [] },
    canvas: {
      styles: [
        "https://fonts.googleapis.com/css2?family=Fira+Sans:wght@400;500;600;700&display=swap",
        "./responsive.css",
      ],
    },
  });
  window.pageEditor = editor;

  const blocks = editor.BlockManager;
  blocks.add("heading", {
    label: "Heading",
    category: "Content",
    content: '<h2 style="margin: 0 0 1rem;">New heading</h2>',
  });
  blocks.add("paragraph", {
    label: "Text",
    category: "Content",
    content: '<p style="line-height: 1.7;">Add your text here.</p>',
  });
  blocks.add("list", {
    label: "List",
    category: "Content",
    content: "<ul><li>First item</li><li>Second item</li></ul>",
  });
  blocks.add("callout", {
    label: "Callout",
    category: "Sections",
    content: '<p class="article-note">Add a helpful note here.</p>',
  });
  blocks.add("accordion", {
    label: "Accordion",
    category: "Sections",
    content:
      '<details><summary>New section</summary><div class="dropdown-body"><p>Add content here.</p></div></details>',
  });
  blocks.add("section", {
    label: "Section",
    category: "Sections",
    content:
      '<section style="padding: 3rem 1.5rem;"><div style="max-width: 42rem; margin: 0 auto;"><h2>New section</h2><p>Add content here.</p></div></section>',
  });
  blocks.add("button", {
    label: "Link button",
    category: "Content",
    content:
      '<a href="#" style="display: inline-block; padding: .7rem 1rem; border-radius: 5px; background: #1769aa; color: white; text-decoration: none;">Learn more</a>',
  });
  blocks.add("link", {
    label: "Link",
    category: "Content",
    content: '<a href="#">Edit this link</a>',
  });
  blocks.add("quote", {
    label: "Quote",
    category: "Content",
    content:
      '<blockquote style="margin: 1.5rem 0; padding: 1rem 1.25rem; border-left: 3px solid #397eaf; background: #eef4f8;"><p style="margin: 0 0 .6rem;">Add a quotation here.</p><cite>— Source</cite></blockquote>',
  });
  blocks.add("divider", {
    label: "Divider",
    category: "Content",
    content: '<hr style="margin: 2rem 0; border: 0; border-top: 1px solid #ccd6e0;" />',
  });
  blocks.add("spacer", {
    label: "Spacer",
    category: "Layout & media",
    content: '<div style="height: 3rem;" aria-hidden="true"></div>',
  });
  blocks.add("image", {
    label: "Image",
    category: "Layout & media",
    content: {
      type: "image",
      attributes: { alt: "Describe this image" },
      style: {
        display: "block",
        width: "100%",
        height: "auto",
        "aspect-ratio": "auto",
        "object-fit": "contain",
      },
    },
  });
  blocks.add("video", {
    label: "Video",
    category: "Layout & media",
    content: {
      type: "video",
      attributes: { controls: true },
      style: { display: "block", width: "100%", "min-height": "18rem", background: "#111319" },
    },
  });
  blocks.add("embed", {
    label: "Embed",
    category: "Layout & media",
    content:
      '<iframe src="about:blank" title="Embedded content" style="display: block; width: 100%; min-height: 24rem; border: 1px solid #ccd6e0;"></iframe>',
  });
  blocks.add("one-column", {
    label: "1 column",
    category: "Layout & media",
    content:
      '<div class="responsive-row" style="padding: 1rem;"><div style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Drop content here</div></div>',
  });
  blocks.add("two-columns", {
    label: "2 columns",
    category: "Layout & media",
    content:
      '<div class="responsive-row" style="padding: 1rem;"><div style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Column 1</div><div style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Column 2</div></div>',
  });
  blocks.add("three-columns", {
    label: "3 columns",
    category: "Layout & media",
    content:
      '<div class="responsive-row" style="padding: 1rem;"><div style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Column 1</div><div style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Column 2</div><div style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Column 3</div></div>',
  });
  blocks.add("sidebar-columns", {
    label: "2/3 + 1/3",
    category: "Layout & media",
    content:
      '<div class="responsive-row" style="padding: 1rem;"><div class="column-wide" style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Main column</div><div style="min-height: 5rem; padding: 1rem; border: 1px dashed #9bb7d0;">Side column</div></div>',
  });
  blocks.add("hero", {
    label: "Hero",
    category: "Sections",
    content:
      '<section style="padding: 5rem 1.5rem; background: #202734; color: #fff; text-align: center;"><div style="max-width: 48rem; margin: 0 auto;"><p style="color: #8bc7ef; font-weight: 600;">Eyebrow</p><h2 style="margin: 0 0 1rem; font-size: 2.4rem;">A strong page headline</h2><p style="margin: 0 auto 1.5rem; max-width: 38rem; line-height: 1.7;">Add a concise explanation of what this section is about.</p><a href="#" style="display: inline-block; padding: .75rem 1rem; border-radius: 5px; background: #397eaf; color: #fff; text-decoration: none;">Get started</a></div></section>',
  });
  blocks.add("card", {
    label: "Card",
    category: "Sections",
    content:
      '<div style="padding: 1.5rem; border: 1px solid #ccd6e0; border-radius: 8px; background: #fff;"><h3 style="margin-top: 0;">Card title</h3><p style="margin-bottom: 0; line-height: 1.6;">Add supporting content here.</p></div>',
  });
  blocks.add("image-text", {
    label: "Image + text",
    category: "Sections",
    content:
      '<section class="responsive-row responsive-media" style="padding: 3rem 1.5rem;"><img src="https://placehold.co/720x480/e4e9ef/526171?text=Choose+an+image" alt="Placeholder" style="border-radius: 8px;" /><div><h2 style="margin-top: 0;">Section title</h2><p style="line-height: 1.7;">Pair an image with explanatory text.</p></div></section>',
  });

  editor.on("load", () => {
    const placeholder = editor.getWrapper().find('[data-bevy-placeholder="true"]')[0];
    if (placeholder) {
      placeholder.set({ selectable: false, hoverable: false, draggable: false, droppable: false, copyable: false, removable: false });
    }
    editor.setDevice("Desktop");
    setStatus("Ready — changes stay local until you save index.html");
  });

  editor.on("update", () => setStatus("Unsaved changes"));
}

function exportSource() {
  const outputDocument = new DOMParser().parseFromString(
    `<body>${editor.getHtml({ cleanId: true })}</body>`,
    "text/html",
  );
  const header = outputDocument.querySelector("header.site-header");
  const article = outputDocument.querySelector("article");
  if (!header || !article) {
    throw new Error("The editor must contain one .site-header and one article element.");
  }

  let output = replaceBetween(sourceHtml, HEADER_START, HEADER_END, indentMarkup(header.outerHTML, 4));
  output = replaceBetween(output, ARTICLE_START, ARTICLE_END, indentMarkup(article.outerHTML, 6));

  const css = editor
    .getCss()
    .replace(/\.visualizer-editor-placeholder[^}]*}\s*/g, "")
    .replace(/\.visualizer-editor-placeholder div[^}]*}\s*/g, "")
    .replace(/\.visualizer-editor-placeholder span[^}]*}\s*/g, "")
    .trim();
  output = output.replace(/(<style>)[\s\S]*?(<\/style>)/, `$1\n${indentMarkup(css, 6)}\n    $2`);
  return output;
}
window.exportPageSource = exportSource;

async function saveSource() {
  try {
    const output = exportSource();
    if ("showSaveFilePicker" in window) {
      const handle = await window.showSaveFilePicker({
        suggestedName: "index.html",
        types: [{ description: "HTML document", accept: { "text/html": [".html"] } }],
      });
      const writable = await handle.createWritable();
      await writable.write(output);
      await writable.close();
      setStatus("Saved index.html");
      return;
    }

    const blob = new Blob([output], { type: "text/html" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "index.html";
    link.click();
    URL.revokeObjectURL(url);
    setStatus("Downloaded index.html — replace web/index.html with it");
  } catch (error) {
    if (error.name !== "AbortError") {
      console.error(error);
      setStatus(error.message, true);
    }
  }
}

document.querySelectorAll("[data-command]").forEach((button) => {
  button.addEventListener("click", () => editor?.runCommand(button.dataset.command));
});

document.querySelectorAll("[data-device]").forEach((button) => {
  button.addEventListener("click", () => {
    editor?.setDevice(button.dataset.device);
    document.querySelectorAll("[data-device]").forEach((item) => item.classList.remove("is-active"));
    button.classList.add("is-active");
  });
});

document.querySelectorAll(".panel-tab").forEach((button) => {
  button.addEventListener("click", () => {
    const panel = button.dataset.panel;
    document.querySelectorAll(".panel-tab").forEach((item) => item.classList.toggle("is-active", item === button));
    document.querySelectorAll(".panel-view").forEach((view) => {
      view.classList.toggle("is-active", view.dataset.panelView === panel);
    });
  });
});

document.querySelector("#export-html").addEventListener("click", saveSource);
document.querySelector("#reload-source").addEventListener("click", () => window.location.reload());

fetch(SOURCE_URL, { cache: "no-store" })
  .then((response) => {
    if (!response.ok) throw new Error(`Could not load ${SOURCE_URL} (${response.status})`);
    return response.text();
  })
  .then((source) => {
    sourceHtml = source;
    initializeEditor(source);
  })
  .catch((error) => {
    console.error(error);
    setStatus(`${error.message}. Serve the web directory over HTTP; do not open editor.html as a file.`, true);
  });
