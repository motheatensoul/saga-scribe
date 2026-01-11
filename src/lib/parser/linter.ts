import { linter, type Diagnostic } from "@codemirror/lint";
import { 
    validateXml, 
    compileDsl, 
    type ValidationResult, 
    type ValidationError 
} from "$lib/tauri";
import { templateStore } from "$lib/stores/template";
import { entityStore } from "$lib/stores/entities";
import { validationStore } from "$lib/stores/validation";
import { get } from "svelte/store";

// Debounce timer
let validationTimeout: ReturnType<typeof setTimeout> | null = null;
const DEBOUNCE_MS = 1000;

export const teiLinter = linter(async (view) => {
    // Clear any existing timeout to restart debounce
    if (validationTimeout) {
        clearTimeout(validationTimeout);
    }

    return new Promise<Diagnostic[]>((resolve) => {
        validationTimeout = setTimeout(async () => {
            const content = view.state.doc.toString();
            const diagnostics: Diagnostic[] = [];

            if (!content.trim()) {
                validationStore.setResult(null);
                resolve([]);
                return;
            }

            // 1. Entity Validation
            // Check for entity patterns :name: and verify they exist in loaded entities
            const entStore = get(entityStore);
            if (entStore.loaded) {
                const entityRegex = /:([a-zA-Z][a-zA-Z0-9]*):/g;
                let match;
                while ((match = entityRegex.exec(content)) !== null) {
                    const name = match[1];
                    // Start position in document
                    const from = match.index;
                    const to = from + match[0].length;
                    
                    if (!entStore.entities[name]) {
                        diagnostics.push({
                            from,
                            to,
                            severity: "error",
                            message: `Unknown entity: :${name}:`
                        });
                    }
                }
            }

            try {
                // Get active template to determine schema and structure
                const store = get(templateStore);
                const activeTemplate = store.active;

                let header = `<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt><title>Validation Draft</title></titleStmt>
      <publicationStmt><p>Draft</p></publicationStmt>
      <sourceDesc><p>Draft</p></sourceDesc>
    </fileDesc>
  </teiHeader>
  <text><body><p>`;
                let footer = `</p></body></text></TEI>`;
                
                let schemaId = "tei-p5";

                if (activeTemplate) {
                    // Use template's header/footer if possible, or construct one compatible
                    // But parsing raw header strings to find line count is tricky for error mapping.
                    // For now, let's stick to the minimal header but ensure namespaces are correct.
                    // If the template is MENOTA, we need the menota namespace.
                    
                    if (activeTemplate.id === "menota" || activeTemplate.validationSchemaId === "menota-p5") {
                         header = `<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0" xmlns:me="http://www.menota.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt><title>Validation Draft</title></titleStmt>
      <publicationStmt><p>Draft</p></publicationStmt>
      <sourceDesc><p>Draft</p></sourceDesc>
    </fileDesc>
  </teiHeader>
  <text><body><p>`;
                    }
                    
                    if (activeTemplate.validationSchemaId) {
                        schemaId = activeTemplate.validationSchemaId;
                    }
                }
                
                const xml = await compileDsl(content, header, footer, {
                    wordWrap: false, // try to preserve structure
                    autoLineNumbers: false
                });

                // 2. Validate XML
                const result = await validateXml(xml, schemaId);
                
                // Update global store for other components (like XML preview)
                validationStore.setResult(result);

                // 3. Map errors to diagnostics
                if (!result.valid) {
                    for (const err of result.errors) {
                         let from = 0;
                         let to = 0;
                         let message = err.message;

                         // Rough heuristic for line mapping:
                         // The header has 11 lines. 
                         // If error line > 11, we subtract 11 to get roughly the DSL line.
                         if (err.line && err.line > 11) {
                             const dslLine = err.line - 11;
                             // Try to find position in document
                             try {
                                 // lines are 1-based in error, 1-based in doc.line()
                                 if (dslLine <= view.state.doc.lines) {
                                     const lineInfo = view.state.doc.line(dslLine);
                                     from = lineInfo.from;
                                     to = lineInfo.to;
                                 }
                             } catch (e) {
                                 // Line out of bounds
                             }
                         }

                         diagnostics.push({
                            from,
                            to,
                            severity: err.isWarning ? "warning" : "error",
                            message: `[${result.schemaName}] ${message} (XML Line ${err.line || '?'})`
                         });
                    }
                }
            } catch (e) {
                console.error("Linter error:", e);
                diagnostics.push({
                    from: 0,
                    to: 0,
                    severity: "error",
                    message: `Linter failed: ${e}`
                });
            }
            
            resolve(diagnostics);
        }, DEBOUNCE_MS);
    });
});
