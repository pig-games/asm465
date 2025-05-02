.cpu "4510"

* = $2001
    .dsection boot

* = $2020
entry
    .dsection init
    .dsection main
    .dsection screen
    .dsection parser

    .align
BasePage
    .logical $0000
    .dsection bp
    .cerror * > $100, "Out of DP space"
    .endlogical

    .align
    .dsection data