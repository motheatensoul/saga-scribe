<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:tei="http://www.tei-c.org/ns/1.0"
    xmlns:me="http://www.menota.org/ns/1.0"
    exclude-result-prefixes="tei me">

    <xsl:output method="html" encoding="UTF-8" indent="yes"/>

    <!-- Root -->
    <xsl:template match="/">
        <div class="prose max-w-none leading-loose">
            <xsl:apply-templates/>
        </div>
    </xsl:template>

    <!-- Structural elements -->
    <xsl:template match="tei:teiHeader | teiHeader">
        <!-- Header is hidden but preserved for metadata access -->
        <div class="bg-base-200 p-4 mb-4 rounded-lg text-sm hidden">
            <h3 class="font-bold">Metadata</h3>
            <xsl:apply-templates/>
        </div>
    </xsl:template>

    <xsl:template match="tei:text | text">
        <div class="transcription">
            <xsl:apply-templates/>
        </div>
    </xsl:template>

    <xsl:template match="tei:body | body">
        <xsl:apply-templates/>
    </xsl:template>

    <xsl:template match="tei:div | div">
        <div class="section mb-4">
            <xsl:apply-templates/>
        </div>
    </xsl:template>

    <!-- Whether the transcription is line by line: then line breaks (<lb/>)
         decide the layout, and paragraphs, which often start mid-line, run inline -->
    <xsl:variable name="lineBased" select="boolean(//*[local-name()='body']//*[local-name()='lb'])"/>

    <xsl:template match="tei:p | p">
        <xsl:choose>
            <xsl:when test="$lineBased and ancestor::*[local-name()='body']">
                <!-- A pilcrow marks where each paragraph starts -->
                <span class="paragraph">
                    <span class="paragraph-mark" aria-hidden="true">¶</span>
                    <xsl:apply-templates/>
                </span>
            </xsl:when>
            <xsl:otherwise>
                <p class="mb-2">
                    <xsl:apply-templates/>
                </p>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:template>

    <xsl:template match="tei:head | head">
        <h2 class="text-xl font-bold mb-2 text-primary">
            <xsl:apply-templates/>
        </h2>
    </xsl:template>

    <!-- Line breaks, also inside words (mid-word breaks, "~//" in the DSL) -->
    <xsl:template match="tei:lb | lb" mode="word-content">
        <xsl:apply-templates select="."/>
    </xsl:template>

    <xsl:template match="tei:lb | lb">
        <br/>
        <xsl:if test="@n">
            <span class="line-number text-xs opacity-40 font-mono mr-2 inline-block min-w-[2rem] text-right select-none">
                <xsl:value-of select="@n"/>
            </span>
        </xsl:if>
    </xsl:template>

    <!-- Page breaks -->
    <xsl:template match="tei:pb | pb">
        <div class="pagebreak block my-6 text-center">
            <span class="page-indicator inline-block px-3 py-1 text-sm opacity-40 font-mono border border-dashed border-current rounded">
                <xsl:value-of select="@n"/>
            </span>
        </div>
    </xsl:template>

    <!-- Word elements with MENOTA multi-level structure -->
    <xsl:template match="tei:w | w">
        <!-- Get diplomatic text for data attribute - use string() to get all descendant text -->
        <xsl:variable name="facs" select="(.//me:facs | .//*[local-name()='facs'])[1]"/>
        <xsl:variable name="dipl" select="(.//me:dipl | .//*[local-name()='dipl'])[1]"/>
        <xsl:variable name="norm" select="(.//me:norm | .//*[local-name()='norm'])[1]"/>
        <xsl:variable name="facsText" select="string($facs)"/>
        <xsl:variable name="diplText" select="string($dipl)"/>
        <xsl:variable name="normText" select="string($norm)"/>

        <!-- The level shown: facsimile, else diplomatic, else normalized, else the word itself -->
        <xsl:variable name="level" select="
            $facs[normalize-space($facsText)]
            | $dipl[not(normalize-space($facsText)) and normalize-space($diplText)]
            | $norm[not(normalize-space($facsText)) and not(normalize-space($diplText)) and normalize-space($normText)]
            | self::*[not(normalize-space($facsText)) and not(normalize-space($diplText)) and not(normalize-space($normText))]"/>
        <xsl:variable name="breaks" select="$level/*[local-name()='lb']"/>
        <xsl:variable name="title">
            <xsl:if test="@lemma">
                <xsl:value-of select="@lemma"/>
                <xsl:if test="@me:msa">
                    <xsl:text> (</xsl:text>
                    <xsl:value-of select="@me:msa"/>
                    <xsl:text>)</xsl:text>
                </xsl:if>
            </xsl:if>
        </xsl:variable>

        <!-- A word broken across lines is shown as one box per line, each line
             ending in a hyphen; the parts after a break are marked as continuations -->
        <xsl:call-template name="word-part">
            <xsl:with-param name="nodes" select="$level/node()[not(self::*[local-name()='lb'])][count(preceding-sibling::*[local-name()='lb']) = 0]"/>
            <xsl:with-param name="diplText" select="$diplText"/>
            <xsl:with-param name="facsText" select="$facsText"/>
            <xsl:with-param name="title" select="$title"/>
            <xsl:with-param name="broken" select="count($breaks) &gt; 0"/>
        </xsl:call-template>
        <xsl:for-each select="$breaks">
            <xsl:variable name="index" select="position()"/>
            <xsl:apply-templates select="."/>
            <xsl:call-template name="word-part">
                <xsl:with-param name="nodes" select="$level/node()[not(self::*[local-name()='lb'])][count(preceding-sibling::*[local-name()='lb']) = $index]"/>
                <xsl:with-param name="diplText" select="$diplText"/>
                <xsl:with-param name="facsText" select="$facsText"/>
                <xsl:with-param name="title" select="$title"/>
                <xsl:with-param name="broken" select="$index &lt; count($breaks)"/>
                <xsl:with-param name="continuation" select="true()"/>
            </xsl:call-template>
        </xsl:for-each>
        <xsl:text> </xsl:text>
    </xsl:template>

    <!-- One box of a word: the whole word, or its part on one line -->
    <xsl:template name="word-part">
        <xsl:param name="nodes"/>
        <xsl:param name="diplText"/>
        <xsl:param name="facsText"/>
        <xsl:param name="title"/>
        <xsl:param name="broken" select="false()"/>
        <xsl:param name="continuation" select="false()"/>
        <!-- An empty part (e.g. after a break at the very end) gets no box -->
        <xsl:if test="not($continuation) or normalize-space(string($nodes)) or $nodes/self::*">
            <span class="word"
                  data-word-index="0"
                  data-diplomatic="{normalize-space($diplText)}"
                  data-facsimile="{normalize-space($facsText)}">
                <xsl:if test="$continuation">
                    <xsl:attribute name="data-word-continuation">true</xsl:attribute>
                </xsl:if>
                <xsl:if test="string($title)">
                    <xsl:attribute name="title"><xsl:value-of select="$title"/></xsl:attribute>
                </xsl:if>
                <!-- Apply templates to preserve <c> element structure for styling -->
                <xsl:apply-templates select="$nodes" mode="word-content"/>
            </span>
            <xsl:if test="$broken">
                <span class="word-break-mark" aria-hidden="true">-</span>
            </xsl:if>
        </xsl:if>
    </xsl:template>

    <!-- Mode for extracting word content including from nested elements like <c> -->
    <!-- Default: pass through to children -->
    <xsl:template match="*" mode="word-content">
        <xsl:apply-templates mode="word-content"/>
    </xsl:template>

    <!-- Character elements: preserve as styled spans for initials, colors, etc. -->
    <xsl:template match="tei:c | c" mode="word-content">
        <span class="char">
            <xsl:if test="@type">
                <xsl:attribute name="data-type"><xsl:value-of select="@type"/></xsl:attribute>
                <xsl:attribute name="class">char char-<xsl:value-of select="@type"/></xsl:attribute>
            </xsl:if>
            <xsl:if test="@rend">
                <xsl:attribute name="data-rend"><xsl:value-of select="@rend"/></xsl:attribute>
            </xsl:if>
            <xsl:apply-templates mode="word-content"/>
        </span>
    </xsl:template>

    <!-- Text nodes: output directly -->
    <xsl:template match="text()" mode="word-content">
        <xsl:value-of select="."/>
    </xsl:template>

    <!-- Punctuation -->
    <xsl:template match="tei:pc | pc">
        <span class="punctuation">
            <xsl:choose>
                <xsl:when test="normalize-space(.//me:facs)">
                    <xsl:apply-templates select=".//me:facs[1]/node()" mode="word-content"/>
                </xsl:when>
                <xsl:when test="normalize-space(.//*[local-name()='facs'])">
                    <xsl:apply-templates select=".//*[local-name()='facs'][1]/node()" mode="word-content"/>
                </xsl:when>
                <xsl:when test="normalize-space(.//me:dipl)">
                    <xsl:apply-templates select=".//me:dipl[1]/node()" mode="word-content"/>
                </xsl:when>
                <xsl:when test="normalize-space(.//*[local-name()='dipl'])">
                    <xsl:apply-templates select=".//*[local-name()='dipl'][1]/node()" mode="word-content"/>
                </xsl:when>
                <xsl:when test="normalize-space(.//me:norm)">
                    <xsl:apply-templates select=".//me:norm[1]/node()" mode="word-content"/>
                </xsl:when>
                <xsl:when test="normalize-space(.//*[local-name()='norm'])">
                    <xsl:apply-templates select=".//*[local-name()='norm'][1]/node()" mode="word-content"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:value-of select="."/>
                </xsl:otherwise>
            </xsl:choose>
        </span>
        <xsl:text> </xsl:text>
    </xsl:template>

    <!-- Editorial interventions -->
    <xsl:template match="tei:del | del">
        <del class="decoration-error decoration-2 bg-error/10 px-0.5 rounded">
            <xsl:apply-templates/>
        </del>
    </xsl:template>

    <xsl:template match="tei:add | add">
        <ins class="decoration-success decoration-2 bg-success/10 px-0.5 rounded">
            <xsl:apply-templates/>
        </ins>
    </xsl:template>

    <xsl:template match="tei:gap | gap">
        <span class="text-base-content/50 italic">[...]</span>
    </xsl:template>

    <xsl:template match="tei:supplied | supplied">
        <span class="text-base-content/70">
            <xsl:text>&#x27E8;</xsl:text>
            <xsl:apply-templates/>
            <xsl:text>&#x27E9;</xsl:text>
        </span>
    </xsl:template>

    <xsl:template match="tei:unclear | unclear">
        <span class="text-base-content/70 italic">
            <xsl:apply-templates/>
            <xsl:text>?</xsl:text>
        </span>
    </xsl:template>

    <!-- Choices (Abbr/Expan) -->
    <xsl:template match="tei:choice | choice">
        <xsl:choose>
            <!-- If this choice has facs with content, show it -->
            <xsl:when test="normalize-space(.//me:facs)">
                <xsl:apply-templates select=".//me:facs[1]/node()" mode="word-content"/>
            </xsl:when>
            <xsl:when test="normalize-space(.//*[local-name()='facs'])">
                <xsl:apply-templates select=".//*[local-name()='facs'][1]/node()" mode="word-content"/>
            </xsl:when>
            <!-- Fall back to dipl if facs is empty -->
            <xsl:when test="normalize-space(.//me:dipl)">
                <xsl:apply-templates select=".//me:dipl[1]/node()" mode="word-content"/>
            </xsl:when>
            <xsl:when test="normalize-space(.//*[local-name()='dipl'])">
                <xsl:apply-templates select=".//*[local-name()='dipl'][1]/node()" mode="word-content"/>
            </xsl:when>
            <!-- Fall back to norm if dipl is also empty -->
            <xsl:when test="normalize-space(.//me:norm)">
                <xsl:apply-templates select=".//me:norm[1]/node()" mode="word-content"/>
            </xsl:when>
            <xsl:when test="normalize-space(.//*[local-name()='norm'])">
                <xsl:apply-templates select=".//*[local-name()='norm'][1]/node()" mode="word-content"/>
            </xsl:when>
            <!-- Otherwise show abbreviation with hover expansion -->
            <xsl:when test="tei:abbr | abbr">
                <span class="group relative cursor-help border-b border-dotted border-info inline-block">
                    <span class="group-hover:hidden">
                        <xsl:apply-templates select="tei:abbr | abbr"/>
                    </span>
                    <span class="hidden group-hover:inline bg-info/10 px-1 rounded">
                        <xsl:apply-templates select="tei:expan | expan"/>
                    </span>
                </span>
            </xsl:when>
            <xsl:otherwise>
                <xsl:apply-templates/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:template>

    <!-- Entity names -->
    <xsl:template match="tei:name | tei:persName | tei:placeName | name | persName | placeName">
        <span class="text-secondary font-medium">
            <xsl:apply-templates/>
        </span>
    </xsl:template>

    <!-- Highlighting -->
    <xsl:template match="tei:hi[@rend='bold'] | hi[@rend='bold']">
        <strong>
            <xsl:apply-templates/>
        </strong>
    </xsl:template>

    <xsl:template match="tei:hi[@rend='italic'] | hi[@rend='italic']">
        <em>
            <xsl:apply-templates/>
        </em>
    </xsl:template>

    <!-- MENOTA-specific: Character elements -->
    <!-- <c> elements just pass through their text content -->
    <xsl:template match="tei:c | c">
        <xsl:value-of select="."/>
    </xsl:template>

    <!-- MENOTA level elements: apply templates to children to process <c> elements etc. -->
    <xsl:template match="me:facs | *[local-name()='facs']">
        <xsl:apply-templates/>
    </xsl:template>

    <xsl:template match="me:dipl | me:norm | *[local-name()='dipl'] | *[local-name()='norm']">
        <!-- Hidden by default -->
    </xsl:template>

    <!-- Abbreviation markers: wrap in span for consistent font styling -->
    <xsl:template match="me:am | *[local-name()='am']">
        <span class="abbr-marker"><xsl:value-of select="."/></span>
    </xsl:template>

    <!-- Expansion markers: wrap in span for consistent font styling -->
    <xsl:template match="me:ex | *[local-name()='ex']">
        <span class="abbr-expansion"><xsl:value-of select="."/></span>
    </xsl:template>

    <!-- Catch-all for unhandled elements: just process children -->
    <xsl:template match="*">
        <xsl:apply-templates/>
    </xsl:template>

</xsl:stylesheet>
