<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:tei="http://www.tei-c.org/ns/1.0"
    xmlns:me="http://www.menota.org/ns/1.0"
    exclude-result-prefixes="tei me">

    <xsl:output method="html" encoding="UTF-8" indent="yes"/>

    <!-- Root -->
    <xsl:template match="/">
        <div class="prose max-w-none font-serif leading-loose">
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

    <xsl:template match="tei:p | p">
        <p class="mb-2">
            <xsl:apply-templates/>
        </p>
    </xsl:template>

    <xsl:template match="tei:head | head">
        <h2 class="text-xl font-bold mb-2 text-primary">
            <xsl:apply-templates/>
        </h2>
    </xsl:template>

    <!-- Line breaks -->
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
        <!-- Note: word index is assigned by JavaScript post-processing, not XSLT -->
        <!-- Using count(preceding::...) here would be O(n²) and cause hangs on large documents -->
        <!-- Get facsimile text: prefer me:facs, then facs, otherwise whole content -->
        <xsl:variable name="facsText">
            <xsl:choose>
                <xsl:when test=".//me:facs">
                    <xsl:value-of select=".//me:facs"/>
                </xsl:when>
                <xsl:when test=".//*[local-name()='facs']">
                    <xsl:value-of select=".//*[local-name()='facs']"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:value-of select="."/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>
        <!-- Get diplomatic text: prefer me:dipl, then dipl, fallback to facs -->
        <xsl:variable name="diplText">
            <xsl:choose>
                <xsl:when test=".//me:dipl">
                    <xsl:value-of select=".//me:dipl"/>
                </xsl:when>
                <xsl:when test=".//*[local-name()='dipl']">
                    <xsl:value-of select=".//*[local-name()='dipl']"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:value-of select="$facsText"/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>

        <span class="word"
              data-word-index="0"
              data-diplomatic="{normalize-space($diplText)}">
            <xsl:if test="@lemma">
                <xsl:attribute name="title">
                    <xsl:value-of select="@lemma"/>
                    <xsl:if test="@me:msa">
                        <xsl:text> (</xsl:text>
                        <xsl:value-of select="@me:msa"/>
                        <xsl:text>)</xsl:text>
                    </xsl:if>
                </xsl:attribute>
            </xsl:if>
            <xsl:value-of select="normalize-space($facsText)"/>
        </span>
        <xsl:text> </xsl:text>
    </xsl:template>

    <!-- Punctuation -->
    <xsl:template match="tei:pc | pc">
        <xsl:variable name="facsText">
            <xsl:choose>
                <xsl:when test=".//me:facs">
                    <xsl:value-of select=".//me:facs"/>
                </xsl:when>
                <xsl:when test=".//*[local-name()='facs']">
                    <xsl:value-of select=".//*[local-name()='facs']"/>
                </xsl:when>
                <xsl:when test=".//me:dipl">
                    <xsl:value-of select=".//me:dipl"/>
                </xsl:when>
                <xsl:when test=".//*[local-name()='dipl']">
                    <xsl:value-of select=".//*[local-name()='dipl']"/>
                </xsl:when>
                <xsl:when test=".//me:norm">
                    <xsl:value-of select=".//me:norm"/>
                </xsl:when>
                <xsl:when test=".//*[local-name()='norm']">
                    <xsl:value-of select=".//*[local-name()='norm']"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:value-of select="."/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>
        <span class="punctuation">
            <xsl:value-of select="normalize-space($facsText)"/>
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
            <!-- If this choice has facs/dipl/norm, show facs content -->
            <xsl:when test=".//me:facs | .//*[local-name()='facs']">
                <xsl:value-of select=".//me:facs | .//*[local-name()='facs']"/>
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
    <xsl:template match="tei:c | c">
        <xsl:choose>
            <xsl:when test=".//me:facs">
                <xsl:value-of select=".//me:facs"/>
            </xsl:when>
            <xsl:when test=".//*[local-name()='facs']">
                <xsl:value-of select=".//*[local-name()='facs']"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:value-of select="."/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:template>

    <!-- MENOTA level elements: show facs by default, hide others -->
    <xsl:template match="me:facs | *[local-name()='facs']">
        <xsl:value-of select="."/>
    </xsl:template>

    <xsl:template match="me:dipl | me:norm | *[local-name()='dipl'] | *[local-name()='norm']">
        <!-- Hidden by default -->
    </xsl:template>

    <!-- Abbreviation markers: show content -->
    <xsl:template match="me:am | *[local-name()='am']">
        <xsl:value-of select="."/>
    </xsl:template>

    <!-- Expansion markers: show content -->
    <xsl:template match="me:ex | *[local-name()='ex']">
        <xsl:value-of select="."/>
    </xsl:template>

    <!-- Catch-all for unhandled elements: just process children -->
    <xsl:template match="*">
        <xsl:apply-templates/>
    </xsl:template>

</xsl:stylesheet>
