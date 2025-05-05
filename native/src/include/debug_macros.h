.cpu "4510"
.enc "screen"

debug .namespace

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
.endmacro

warningReg .macro reg="a"
    #debug.infoCReg 7, \reg
.endmacro

error .macro str
    #debug.infoC 9, \str
.endmacro

errorReg .macro reg="a"
    #debug.infoCReg 8, \reg
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
.endmacro

errorLn .macro str
    #debug.infoCLn 9, \str
.endmacro

.endnamespace ; def debug