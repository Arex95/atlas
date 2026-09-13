/*
 * Syntax highlighting, tuned to this site's palette.
 *
 * VitePress ships the GitHub themes by default, which bring seven hues of their
 * own — including a second orange (#FFAB70) that competes with the brand one.
 * The page is held together by having a single accent; the code blocks were
 * quietly spending six more.
 *
 * Two hues, on purpose:
 *   - the orange names things (keywords, markdown headings) and nothing else
 *   - one sage carries literals, so a string never reads as code
 *   - everything else is the warm neutral scale, where BRIGHTNESS carries the
 *     hierarchy instead of colour
 *
 * Both faces are measured against the real code-block background, which is
 * --vp-c-bg-alt: #000000 in dark and #ece4d3 in light. The light one is the
 * binding constraint — on that cream a token needs a relative luminance below
 * 0.1345 to clear 4.5:1, which is why these are darker than they look.
 */

type Rule = { scope: string | string[]; settings: { foreground?: string; fontStyle?: string } }

const rules = (palette: Record<string, string>): Rule[] => [
    {
        scope: ['comment', 'punctuation.definition.comment', 'string.comment'],
        settings: { foreground: palette.comment, fontStyle: 'italic' }
    },
    {
        scope: [
            'punctuation',
            'meta.brace',
            'punctuation.definition.tag',
            'punctuation.separator',
            'punctuation.terminator'
        ],
        settings: { foreground: palette.punctuation }
    },
    {
        scope: [
            'keyword',
            'storage',
            'storage.type',
            'storage.modifier',
            'keyword.control',
            'keyword.operator.new',
            'keyword.operator.expression',
            'variable.language',
            'entity.name.tag',
            'markup.heading',
            'entity.name.section'
        ],
        settings: { foreground: palette.accent }
    },
    {
        scope: ['keyword.operator', 'meta.property-name'],
        settings: { foreground: palette.punctuation }
    },
    {
        scope: ['string', 'string.quoted', 'string.template', 'markup.inline.raw', 'markup.raw'],
        settings: { foreground: palette.literal }
    },
    {
        scope: ['constant.character.escape', 'punctuation.definition.string'],
        settings: { foreground: palette.literalDim }
    },
    {
        scope: [
            'constant',
            'constant.numeric',
            'constant.language',
            'support.constant',
            'keyword.other.unit'
        ],
        settings: { foreground: palette.constant }
    },
    {
        scope: [
            'entity.name.function',
            'support.function',
            'meta.function-call',
            'variable.function'
        ],
        settings: { foreground: palette.bright }
    },
    {
        scope: [
            'entity.name.type',
            'entity.name.class',
            'support.type',
            'support.class',
            'entity.other.inherited-class'
        ],
        settings: { foreground: palette.type }
    },
    {
        scope: [
            'variable',
            'variable.other',
            'meta.object-literal.key',
            'support.type.property-name',
            'entity.other.attribute-name'
        ],
        settings: { foreground: palette.variable }
    },
    {
        scope: ['markup.bold', 'markup.list punctuation.definition'],
        settings: { foreground: palette.bright, fontStyle: 'bold' }
    },
    {
        scope: ['markup.italic'],
        settings: { fontStyle: 'italic' }
    },
    {
        scope: ['markup.underline.link', 'string.other.link'],
        settings: { foreground: palette.literal, fontStyle: 'underline' }
    },
    {
        scope: ['markup.inserted', 'meta.diff.header.to-file'],
        settings: { foreground: palette.literal }
    },
    {
        scope: ['markup.deleted', 'meta.diff.header.from-file'],
        settings: { foreground: palette.deleted }
    },
    {
        scope: ['invalid', 'invalid.illegal'],
        settings: { foreground: palette.deleted }
    }
]

const theme = (
    name: string,
    type: 'dark' | 'light',
    background: string,
    palette: Record<string, string>
) => ({
    name,
    type,
    colors: {
        'editor.foreground': palette.base,
        'editor.background': background
    },
    settings: rules(palette),
    tokenColors: rules(palette)
})

export const arexDark = theme('arex-dark', 'dark', '#000000', {
    base: '#d8d4ca',
    bright: '#f0ece2',
    variable: '#c6bfb2',
    type: '#c9c0ae',
    constant: '#d9c9a8',
    punctuation: '#9a9288',
    comment: '#7c766c',
    accent: '#ff6321',
    literal: '#a8b57f',
    literalDim: '#8b9668',
    deleted: '#e06c5a'
})

export const arexLight = theme('arex-light', 'light', '#ece4d3', {
    base: '#16130f',
    bright: '#2a2622',
    variable: '#3a352e',
    type: '#5a4a30',
    constant: '#6b5a3a',
    punctuation: '#4c463e',
    comment: '#605a51',
    accent: '#b34009',
    literal: '#4f5a2e',
    literalDim: '#616b3f',
    deleted: '#8c2f1c'
})
