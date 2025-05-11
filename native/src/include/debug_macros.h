.cpu "4510"
.enc "screen"

debug .namespace
    STATS_ :?=false

setupDebugStats .macro datasection=data
    .if DEBUG_
    .namespace debug
        STATS_ := true
        .section \datasection
            NumWarnings .byte 0
            NumErrors   .byte 0
        .endsection ; data
    .endnamespace
    .endif
.endmacro

infoCReg .macro col=5, reg="a"
    .if DEBUG_
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
        ldz #\col
        jsr setCPrintC
        plq
    .endif
.endmacro

infoCRegDec .macro col=5, reg="a"
    .if DEBUG_
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
        ldz #\col
        stz PrtColour
        sta toDec.In
        ldx #0
        stx toDec.In+1
        jsr toDec
        #ldbcd24 toDec.Out
        jsr cPrintBCD24
        plq
    .endif
.endmacro

infoCRegHex .macro col=5, reg="a"
    .if DEBUG_
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
        ldz #\col
        stz PrtColour
        jsr toHexXY
        txa
        jsr cPrintC
        tya
        jsr cPrintC
        plq
    .endif
.endmacro

infoC .macro col, str
    .if DEBUG_
        #cpr \col, \str
    .endif
.endmacro

info .macro str
    #debug.infoC 5, \str
.endmacro

infoCLn .macro col, str
    .if DEBUG_
        #cprl \col, \str
    .endif
.endmacro

infoLn .macro str
    #debug.infoCLn 5, \str
.endmacro

infoCPtr .macro col, ptr
    .if DEBUG_
        phq
        ldz #\col
        stz PrtColour
        #ldxy \ptr
        jsr print
        plq
    .endif
.endmacro

infoCLnPtr .macro col, ptr
    #debug.infoCPtr \col, \ptr
    #nl
.endmacro

infoPtr .macro ptr
    #debug.infoCPtr 5, \ptr
.endmacro

infoLnPtr .macro ptr
    #debug.infoCLnPtr 5, \ptr
.endmacro

infoReg .macro reg="a"
    #debug.infoCReg 5, \reg
.endmacro

infoRegDec .macro reg="a"
    #debug.infoCRegDec 5, \reg
.endmacro

infoRegHex .macro reg="a"
    #debug.infoCRegHex 5, \reg
.endmacro

warning .macro str
    #debug.infoC 7, \str
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

warningLn .macro str
    #debug.infoCLn 7, \str
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

warningPtr .macro ptr
    #debug.infoCPtr 7, \ptr
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

warningLnPtr .macro ptr
    #debug.infoCLnPtr 7, \ptr
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

warningReg .macro reg="a"
    #debug.infoCReg 7, \reg
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

warningRegDec .macro reg="a"
    #debug.infoCRegDec 7, \reg
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

warningRegHex .macro reg="a"
    #debug.infoCRegHex 7, \reg
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

error .macro str
    #debug.infoC 9, \str
    .if DEBUG_ && debug.STATS_
        inc debug.NumErrors
    .endif
.endmacro

errorLn .macro str
    #debug.infoCLn 9, \str
    .if DEBUG_ && debug.STATS_
        inc debug.NumErrors
    .endif
.endmacro

errorPtr .macro ptr
    #debug.infoCPtr 9, \ptr
    .if DEBUG_ && debug.STATS_
        inc debug.NumErrors
    .endif
.endmacro

errorLnPtr .macro ptr
    #debug.infoCLnPtr 9, \ptr
    .if DEBUG_ && debug.STATS_
        inc debug.NumErrors
    .endif
.endmacro

errorReg .macro reg="a"
    #debug.infoCReg 8, \reg
    .if DEBUG_ && debug.STATS_
        inc debug.NumErrors
    .endif
.endmacro

errorRegDec .macro reg="a"
    #debug.infoCRegDec 9, \reg
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

errorRegHex .macro reg="a"
    #debug.infoCRegHex 9, \reg
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

numWarnings .macro
    .if DEBUG_ && debug.STATS_
        phq
        #debug.infoC  7, "Number of warnings: "
        lda debug.NumWarnings
        #debug.infoCRegDec 7
        #nl
        plq
    .endif
.endmacro

numErrors .macro
    .if DEBUG_ && debug.STATS_
        phq
        #debug.infoC 9, "Number of errors: "
        lda debug.NumErrors
        #debug.infoCRegDec 9
        #nl
        plq
    .endif
.endmacro

Stats .macro
    #debug.infoLn "Debug stats:"
    #debug.numWarnings
    #debug.numErrors
.endmacro

.endnamespace ; def debug