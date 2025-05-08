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
        pha
        phx
        phy
        phz
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
        plz
        ply
        plx
        pla
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

infoReg .macro reg="a"
    #debug.infoCReg 5, \reg
.endmacro

warning .macro str
    #debug.infoC 7, \str
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

error .macro str
    #debug.infoC 9, \str
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

infoCLn .macro col, str
    .if DEBUG_
        #cprl \col, \str
    .endif
.endmacro

infoLn .macro str
    #debug.infoCLn 5, \str
.endmacro

warningLn .macro str
    #debug.infoCLn 7, \str
    .if DEBUG_ && debug.STATS_
        inc debug.NumWarnings
    .endif
.endmacro

errorLn .macro str
    #debug.infoCLn 9, \str
    .if DEBUG_ && debug.STATS_
        inc debug.NumErrors
    .endif
.endmacro

numWarnings .macro
    .if DEBUG_ && debug.STATS_
        phq
        #debug.infoC  7, "Number of warnings: "
        lda debug.NumWarnings
        #debug.infoCReg 7
        #nl
        plq 
    .endif
.endmacro

numErrors .macro
    .if DEBUG_ && debug.STATS_
        phq
        #debug.infoC 9, "Number of errors: "
        lda debug.NumErrors
        #debug.infoCReg 9
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