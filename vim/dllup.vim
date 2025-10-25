" Vim syntax file
" Based on vim syntax file for Markdown by Tim Pope
" Language:     dllup
" Maintainer:   Daniel L. Lu daniel@lawrence.lu
" Filenames:    *.dllup *.dllu
" Last Change:  2025-10-25

if exists("b:current_syntax")
  finish
endif

if !exists('main_syntax')
  let main_syntax = 'dllup'
endif

runtime! syntax/html.vim
unlet! b:current_syntax

if !exists('g:dllup_fenced_languages')
  let g:dllup_fenced_languages = []
endif
for s:type in map(copy(g:dllup_fenced_languages),'matchstr(v:val,"[^=]*$")')
  if s:type =~ '\.'
    let b:{matchstr(s:type,'[^.]*')}_subtype = matchstr(s:type,'\.\zs.*')
  endif
  exe 'syn include @dllupHighlight'.substitute(s:type,'\.','','g').' syntax/'.matchstr(s:type,'[^.]*').'.vim'
  unlet! b:current_syntax
endfor
unlet! s:type

syn sync minlines=10
syn case ignore

syn match dllupValid '[<>]\c[a-z/$!]\@!'
syn match dllupValid '&\%(#\=\w*;\)\@!'

syn match dllupLineStart "^[<@]\@!" nextgroup=@dllupBlock,htmlSpecialChar

syn cluster dllupBlock contains=dllupH1,dllupH2,dllupH3,dllupH4,dllupH5,dllupH6,dllupBlockquote,dllupListMarker,dllupOrderedListMarker,dllupCodeBlock,dllupRule,dllupDisplayMath,dllupPic,dllupRawBlock,dllupHeaderDivider,dllupBigButton,dllupTableRow,dllupTableSeparator
syn cluster dllupInline contains=dllupLineBreak,dllupLinkText,dllupItalic,dllupBold,dllupCode,dllupEscape,@htmlTop,dllupError,dllupInlineMath,dllupCite,dllupRef

syn region dllupH1 matchgroup=dllupHeadingDelimiter start="##\@!"      end="#*\s*$" keepend oneline contains=@dllupInline,dllupAutomaticLink contained
syn region dllupH2 matchgroup=dllupHeadingDelimiter start="###\@!"     end="#*\s*$" keepend oneline contains=@dllupInline,dllupAutomaticLink contained
syn region dllupH3 matchgroup=dllupHeadingDelimiter start="####\@!"    end="#*\s*$" keepend oneline contains=@dllupInline,dllupAutomaticLink contained
syn region dllupH4 matchgroup=dllupHeadingDelimiter start="#####\@!"   end="#*\s*$" keepend oneline contains=@dllupInline,dllupAutomaticLink contained
syn region dllupH5 matchgroup=dllupHeadingDelimiter start="######\@!"  end="#*\s*$" keepend oneline contains=@dllupInline,dllupAutomaticLink contained
syn region dllupH6 matchgroup=dllupHeadingDelimiter start="#######\@!" end="#*\s*$" keepend oneline contains=@dllupInline,dllupAutomaticLink contained

syn region dllupCodeBlock start="^\~\~\~\~$" end="^\~\~\~\~$" contains=dllupCodeInfo contained
syn region dllupCodeBlock start="^\~\~\~$" end="^\~\~\~$" contains=dllupCodeInfo contained

syn region dllupBlockquote start="^> " end="\n\n" contains=@dllupInline contained
syn region dllupDisplayMath start="^\$ " end="\n\n" contained
syn region dllupPic start="^pic " end="\n\n" contains=dllupPicKeyword,dllupPicUrl,dllupPicAlt,dllupPicColon,dllupPicCaption contained
syn match dllupPicKeyword "^pic" contained
syn match dllupPicUrl "\%(^pic\s\+\)\@<=\S\+" contained
syn match dllupPicAlt "\%(^pic\s\+\S\+\s\+\)\@<=.\{-}\ze\s:\s" contained
syn match dllupPicColon ":\ze\s\+" contained containedin=dllupPic
syn region dllupPicCaption start=":\s\+\zs" end="$" contains=@dllupInline,dllupAutomaticLink keepend contained
syn region dllupRawBlock matchgroup=dllupFence start="^???\s*$" end="^???\s*$" keepend contained
syn match dllupHeaderDivider "^===\s*$" contained
syn region dllupBigButton start="^::\s" end="$" contains=@dllupInline,dllupBigButtonUrl,dllupBigButtonMarker keepend transparent contained
syn match dllupBigButtonMarker "^::" contained
syn match dllupBigButtonUrl "\S\+$" contained
syn match dllupTableSeparator "^\s*|[-| \t]*$" contained
syn region dllupTableRow start="^\s*|\s" end="$" contains=@dllupInline,dllupTablePipe keepend contained
syn match dllupTablePipe "|" contained

" TODO: real nesting
syn match dllupListMarker "\%(\t\| \{0,4\}\)[*]\+\%(\s\+\S\)\@=" contained
syn match dllupOrderedListMarker "\%(\t\| \{0,4}\)\<\d\+\.\%(\s\+\S\)\@=" contained

syn match dllupRule "\* *\* *\*[ *]*$" contained
syn match dllupRule "- *- *-[ -]*$" contained

syn match dllupLineBreak " \{2,\}$"

syn region dllupIdDeclaration matchgroup=dllupLinkDelimiter start="^ \{0,3\}!\=\[" end="\]:" oneline keepend nextgroup=dllupUrl skipwhite
syn match dllupUrl "\S\+" nextgroup=dllupUrlTitle skipwhite contained
syn region dllupUrl matchgroup=dllupUrlDelimiter start="<" end=">" oneline keepend nextgroup=dllupUrlTitle skipwhite contained
syn region dllupUrlTitle matchgroup=dllupUrlTitleDelimiter start=+"+ end=+"+ keepend contained
syn region dllupUrlTitle matchgroup=dllupUrlTitleDelimiter start=+'+ end=+'+ keepend contained
syn region dllupUrlTitle matchgroup=dllupUrlTitleDelimiter start=+(+ end=+)+ keepend contained

syn region dllupLinkText matchgroup=dllupLinkTextDelimiter start="!\=\[\%(\_[^]]*]\%( \=[[(]\)\)\@=" end="\]\%( \=[[(]\)\@=" keepend nextgroup=dllupLink,dllupId skipwhite contains=@dllupInline,dllupLineStart
syn region dllupLink matchgroup=dllupLinkDelimiter start="(" end=")" contains=dllupUrl keepend contained
syn region dllupId matchgroup=dllupIdDelimiter start="\[" end="\]" keepend contained
syn region dllupAutomaticLink matchgroup=dllupUrlDelimiter start="<\%(\w\+:\|[[:alnum:]_+-]\+@\)\@=" end=">" keepend oneline

syn region dllupCite start="(#" end=")" keepend oneline contains=dllupLineStart
syn region dllupRef start="\[\s*#" end="\]" keepend oneline contains=dllupLineStart

syn region dllupInlineMath start="\$" skip="\\\$" end="\$" keepend oneline contains=dllupLineStart
syn region dllupItalic start="\S\@<=_\|_\S\@=" end="\S\@<=_\|_\S\@=" keepend oneline contains=dllupLineStart
syn region dllupBold start="\S\@<=\*\*\|\*\*\S\@=" end="\S\@<=\*\*\|\*\*\S\@=" keepend oneline contains=dllupLineStart,dllupItalic
syn region dllupCode matchgroup=dllupCodeDelimiter start="[^\\]`" end="`" keepend contains=dllupLineStart

if main_syntax ==# 'dllup'
  for s:type in g:dllup_fenced_languages
    exe 'syn region dllupHighlight'.substitute(matchstr(s:type,'[^=]*$'),'\..*','','').' matchgroup=dllupCodeDelimiter start="^\s*```'.matchstr(s:type,'[^=]*').'\>.*$" end="^\s*```\ze\s*$" keepend contains=@dllupHighlight'.substitute(matchstr(s:type,'[^=]*$'),'\.','','g')
  endfor
  unlet! s:type
endif

syn match dllupEscape "\\[][\\`*_{}()#+.!-]"
syn match dllupCodeInfo "^\s*lang\s\+\S\+" contained

hi def link dllupH1                    htmlH1
hi def link dllupH2                    htmlH2
hi def link dllupH3                    htmlH3
hi def link dllupH4                    htmlH4
hi def link dllupH5                    htmlH5
hi def link dllupH6                    htmlH6
hi def link dllupHeadingRule           dllupRule
hi def link dllupHeadingDelimiter      Delimiter
hi def link dllupOrderedListMarker     dllupListMarker
hi def link dllupListMarker            htmlTagName
hi def link dllupBlockquote            String
hi def link dllupPic                   Label
hi def link dllupRule                  PreProc
hi def link dllupHeaderDivider         Delimiter
hi def link dllupRawBlock              Comment
hi def link dllupFence                 PreProc
hi def link dllupBigButton             Statement
hi def link dllupBigButtonMarker       Statement
hi def link dllupBigButtonUrl          Underlined
hi def link dllupTableSeparator        Identifier
hi def link dllupTableRow              Normal
hi def link dllupTablePipe             Delimiter
hi def link dllupInlineMath            dllupDisplayMath
hi def dllupDisplayMath           term=italic cterm=italic gui=italic ctermfg=blue guifg=blue
hi def link dllupCodeBlock             Comment
hi def link dllupCodeInfo              Identifier
hi def link dllupPicKeyword            Statement
hi def link dllupPicUrl                Identifier
hi def link dllupPicAlt                String
hi def link dllupPicCaption            Normal
hi def link dllupPicColon              Delimiter

hi def link dllupLinkText              htmlLink
hi def link dllupIdDeclaration         Typedef
hi def link dllupId                    Type
hi def link dllupAutomaticLink         dllupUrl
hi def link dllupUrl                   Float
hi def link dllupUrlTitle              String
hi def link dllupRef                   htmlTagName
hi def link dllupCite                  htmlTagName
hi def link dllupCode                  Comment
hi def link dllupIdDelimiter           dllupLinkDelimiter
hi def link dllupUrlDelimiter          htmlTag
hi def link dllupUrlTitleDelimiter     Delimiter

hi def link dllupItalic                htmlItalic
hi def link dllupBold                  htmlBold
hi def link dllupCodeDelimiter         Delimiter

hi def link dllupEscape                Special
hi def link dllupError                 Error

let b:current_syntax = "dllup"
if main_syntax ==# 'dllup'
  unlet main_syntax
endif

" vim:set sw=2:
