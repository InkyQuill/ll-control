pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import net.inkyquill.lconnect 1.0
import "../code/mini.mjs" as Mini

ColumnLayout {
 id: root
 property string cfg_slotConfig
 property var draft:Mini.parseConfig(cfg_slotConfig)
 property bool ready:false
 ListModel {id:slotsModel;dynamicRoles:true}
 function syncSlots() {
  const config=Mini.parseConfig(cfg_slotConfig)
  if(!config){slotsModel.clear();return}
  for(let i=0;i<6;i++){if(i<slotsModel.count)slotsModel.set(i,{tile:config.slots[i]??{kind:"",source:null}});else slotsModel.append({tile:config.slots[i]??{kind:"",source:null}})}
 }
 Component.onCompleted:{ready=true;syncSlots()}
 onCfg_slotConfigChanged:if(ready)syncSlots()
 property string cfg_languageCache
 property string language:cfg_languageCache==="ru-RU"?"ru-RU":"en-US"
 readonly property var frame:JSON.parse(feed.frameJson)
 Feed {id:feed;active:true;onFrameChanged:root.cfg_languageCache=Mini.languageOf(JSON.parse(frameJson),root.language)}
 function edit(index,kind,source) {
  const next=Mini.parseConfig(cfg_slotConfig)
  if(!next)return
  next.slots[index]=kind?{kind:kind,source:source}:null
  cfg_slotConfig=JSON.stringify(next)
 }
 Controls.Label {text:Mini.text("invalid",root.language);visible:!root.draft;wrapMode:Text.Wrap;Layout.fillWidth:true}
 Repeater {
  model:slotsModel
  ColumnLayout {
   id: slot
   required property int index
   required property var tile
   readonly property var choices:[{kind:"",label:Mini.text("empty",root.language)}].concat(Mini.kinds.map(kind=>({kind:kind,label:Mini.label(kind,root.language)})))
   readonly property var sourceChoices: {
    if(!tile?.kind)return []
    const options=[{id:null,label:Mini.text(tile.kind==="network"?"total":tile.kind==="fps"?"unselected":"shared",root.language)}].concat(Mini.sources(tile.kind,root.frame))
    if(tile.source&&!options.some(option=>option.id===tile.source))options.push({id:tile.source,label:Mini.text("missing",root.language)+": "+tile.source})
    return options
   }
   Layout.fillWidth:true
   Controls.Label {text:Mini.text("slot",root.language)+" "+(slot.index+1);font.bold:true}
   Controls.ComboBox {
    Layout.fillWidth:true
    objectName:"reading"+slot.index
    model:slot.choices;textRole:"label";valueRole:"kind"
    currentIndex:slot.choices.findIndex(option=>option.kind===(slot.tile?slot.tile.kind:""))
    Accessible.name:Mini.text("reading",root.language)+" "+(slot.index+1)
    onActivated:root.edit(slot.index,currentValue,null)
   }
   Controls.ComboBox {
    visible:!!slot.tile?.kind&&slot.tile.kind!=='ram'
    Layout.fillWidth:true
    objectName:"source"+slot.index
    model:slot.sourceChoices;textRole:"label";valueRole:"id"
    currentIndex:slot.sourceChoices.findIndex(option=>option.id===slot.tile?.source)
    Accessible.name:Mini.text("source",root.language)+" "+(slot.index+1)
    onActivated:if(slot.tile)root.edit(slot.index,slot.tile.kind,currentIndex===0?null:currentValue)
   }
  }
 }
 Controls.Button {objectName:"resetSlots";text:Mini.text("reset",root.language);onClicked:root.cfg_slotConfig=JSON.stringify(Mini.defaultConfig())}
 Controls.Label {text:Mini.text("fps",root.language);wrapMode:Text.Wrap;Layout.fillWidth:true}
}
