DEBUG_      :?= false
DBG_FILTER_ :?= []
DBG_TAG_    :?= ""

.if !CROSS465
  .cerror "Using cross465 layout.s but CROSS465 not defined"
.endif

* = $8000
    .dsection boot

entry
    .dsection init
    .dsection main
    .dsection screen
    .dsection util
    .dsection parser
    .align

BasePage
    .logical $0000
    .dsection bp
    .cerror * > $0100, "Out of BP space"
    .endlogical

    .dsection data
