DEBUGMACROS :?= false
.if !DEBUGMACROS
DEBUGMACROS := true

.enc "screen"

; When enabled, debug macros also emit RTST MSG records (and optionally skip
; device console output). The runner defines DEBUG_RTST_ENABLED by default.
DEBUG_RTST_ENABLED :?= 0
DEBUG_RTST_ONLY    :?= 0

.if DEBUG_RTST_ENABLED
.include "test_rtst.h"
.endif

dbg .namespace

setupDebugStats .macro datasection=data
    .if DEBUG_
    .namespace dbg
        .section \datasection
            NumWarnings .byte 0
            NumErrors   .byte 0
        .endsection ; \datasection
    .endnamespace ; dbg
    .endif
.endmacro

setFilter .macro
    .if DEBUG_
        DBG_FILTER_ ::= [\@]
    .endif
.endmacro

resetFilter .macro
    .if DEBUG_
        DBG_FILTER_ ::= []
    .endif
.endmacro

setTag .macro tag
    .if DEBUG_
        DBG_TAG_ ::= \tag
    .endif
.endmacro

resetTag .macro
    .if DEBUG_
        DBG_TAG_ ::= ""
    .endif
.endmacro

only .macro
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        \@
    .endif
.endmacro

infoCReg .macro col=5, reg="a", pre="", post=""
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        phq
        .switch \reg
        .case "x"
            txa
        .case "y"
            tya
        .case "z"
            tza
        .default
        .endswitch
        .if \pre!=""
            .dbg.infoC \col, \pre
        .endif
        ldx #\col
        jsr setCPrintC
        .if \post!=""
            .dbg.infoC \col, \post
        .endif
        plq
    .endif
.endmacro

infoCRegDec .macro col=5, reg="a", pre="", post=""
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        phq
        .switch \reg
        .case "x"
            txa
        .case "y"
            tya
        .case "z"
            tza
        .default
        .endswitch
        ldx #\col
        stx PrtColour
        .if \pre!=""
            .dbg.infoC \col, \pre
        .endif
        sta toDec.In
        ldx #0
        stx toDec.In+1
        jsr toDec
        .ldbcd24 toDec.Out
        jsr cPrintBCD24
        .if \post!=""
            .dbg.infoC \col, \post
        .endif
        plq
    .endif
.endmacro

infoCRegHex .macro col=5, reg="a", pre="", post=""
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        phq
        .switch \reg
        .case "x"
            txa
        .case "y"
            tya
        .case "z"
            tza
        .default
        .endswitch
        ldx #\col
        stx PrtColour
        .if \pre!=""
            .dbg.infoC \col, \pre
        .endif
        jsr toHexXY
        txa
        sty YStore
        jsr cPrintC
        ldy YStore
        tya
        jsr cPrintC
        .if \post!=""
            .dbg.infoC \col, \post
        .endif
        plq
    .endif
.endmacro

infoCXYHex .macro col=5, pre="", post=""
    .dbg.infoCRegHex \col, "y", \pre
    .dbg.infoCRegHex \col, "x", "", \post
.endmacro

infoC .macro col, str
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        .if !DEBUG_RTST_ONLY
            .cpr \col, \str
        .endif
        .if DEBUG_RTST_ENABLED
msg\@:
            .null \str
            .rtst.logMsg msg\@
        .endif
    .endif
.endmacro

info .macro str
    .dbg.infoC 5, \str
.endmacro

infoCPtr .macro col, ptr
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        .if !DEBUG_RTST_ONLY
            phq
            ldx #\col
            stx PrtColour
            .ldxy \ptr
            jsr print
            plq
        .endif
        .if DEBUG_RTST_ENABLED
            .rtst.logMsg \ptr
        .endif
    .endif
.endmacro

infoCLnPtr .macro col, ptr
    .dbg.infoCPtr \col, \ptr
    .nl
.endmacro

infoPtr .macro ptr
    .dbg.infoCPtr 5, \ptr
.endmacro

infoLnPtr .macro ptr
    .dbg.infoCLnPtr 5, \ptr
.endmacro

infoReg .macro reg="a", pre="", post=""
    .dbg.infoCReg 5, \reg, \pre, \post
.endmacro

infoRegDec .macro reg="a", pre="", post=""
    .dbg.infoCRegDec 5, \reg, \pre, \post
.endmacro

infoRegHex .macro reg="a", pre="", post=""
    .dbg.infoCRegHex 5, \reg, \pre, \post
.endmacro

infoXYHex .macro pre="", post=""
    .dbg.infoCXYHex 5, \pre, \post
.endmacro

warning .macro str
    .dbg.infoC 7, \str
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumWarnings
    .endif
.endmacro

warningPtr .macro ptr
    .dbg.infoCPtr 7, \ptr
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumWarnings
    .endif
.endmacro

warningReg .macro reg="a", pre="", post=""
    .dbg.infoCReg 7, \reg, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumWarnings
    .endif
.endmacro

warningRegDec .macro reg="a", pre="", post=""
    .dbg.infoCRegDec 7, \reg, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumWarnings
    .endif
.endmacro

warningRegHex .macro reg="a", pre="", post=""
    .dbg.infoCRegHex 7, \reg, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumWarnings
    .endif
.endmacro

warningXYHex .macro pre="", post=""
    .dbg.infoCXYHex 7, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumWarnings
    .endif
.endmacro

error .macro str
    .dbg.infoC 9, \str
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumErrors
    .endif
.endmacro

errorPtr .macro ptr
    .dbg.infoCPtr 9, \ptr
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumErrors
    .endif
.endmacro

errorReg .macro reg="a", pre="", post=""
    .dbg.infoCReg 9, \reg, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumErrors
    .endif
.endmacro

errorRegDec .macro reg="a", pre="", post=""
    .dbg.infoCRegDec 9, \reg, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumErrors
    .endif
.endmacro

errorRegHex .macro reg="a", pre="", post=""
    .dbg.infoCRegHex 9, \reg, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumErrors
    .endif
.endmacro

errorXYHex .macro pre="", post=""
    .dbg.infoCXYHex 9, \pre, \post
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        inc dbg.NumErrors
    .endif
.endmacro

numWarnings .macro
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        phq
        .dbg.infoC 7, "Number of warnings: "
        lda dbg.NumWarnings
        .dbg.infoCRegDec 7
        .nl
        plq
    .endif
.endmacro

numErrors .macro
    .if DEBUG_ && (DBG_TAG_ == "" || (DBG_TAG_ in DBG_FILTER_) || ("all" in DBG_FILTER_))
        phq
        .dbg.infoC 9, "Number of errors: "
        lda dbg.NumErrors
        .dbg.infoCRegDec 9
        .nl
        plq
    .endif
.endmacro

Stats .macro
    .dbg.info "Debug stats:!n"
    .dbg.numWarnings
    .dbg.numErrors
.endmacro

.endnamespace ; dbg

.endif
