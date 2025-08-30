DEBUG_      :?= false
DBG_FILTER_ :?= []
DBG_TAG_    :?= ""

.if !ULTIMATE64
  .cerror "Using ultimate64 layout.s but ULTIMATE64 not defined"
.endif

* = $0801
    .dsection boot

* = $0810
entry
    .dsection init
    .dsection main
    .dsection screen
    .dsection util
    .dsection parser
    .align

BasePage
    .logical $0002
    .dsection bp
    .cerror * > $0100, "Out of BP space"
    .endlogical

    .dsection data
