pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasmoid
import org.kde.kirigami as Kirigami
import net.inkyquill.lconnect 1.0
import "../code/mini.mjs" as Mini

PlasmoidItem {
 id: root
 implicitWidth: Kirigami.Units.gridUnit * 24
 implicitHeight: Kirigami.Units.gridUnit * 15
 property string cachedLanguage: Plasmoid.configuration.languageCache === "ru-RU" ? "ru-RU" : "en-US"
 readonly property var frame:JSON.parse(feed.frameJson)
 readonly property var view: Mini.render(root.frame, Plasmoid.configuration.slotConfig, feed.elapsedMs, cachedLanguage)
 preferredRepresentation: Plasmoid.formFactor===PlasmaCore.Types.Planar?fullRepresentation:compactRepresentation
 compactRepresentation: Controls.ToolButton {
  text:root.view.cards.filter(card=>card.available).slice(0,2).map(card=>card.values[0]).join(" · ")||"LL-Control"
  Accessible.name:root.view.status||root.view.cards.filter(card=>card.available).slice(0,2).map(card=>card.title+" "+card.values[0]).join(" · ")||"LL-Control Mini"
  onClicked:root.expanded=!root.expanded
 }
 Feed {
  id: feed
  active: true
  onFrameChanged: {
   const language=Mini.languageOf(JSON.parse(frameJson),root.cachedLanguage)
   if(language!==root.cachedLanguage) {root.cachedLanguage=language;Plasmoid.configuration.languageCache=language}
  }
 }
 fullRepresentation: ColumnLayout {
  spacing: Kirigami.Units.smallSpacing
  Controls.Label {text: root.view.status; visible: text.length>0; wrapMode: Text.Wrap; textFormat: Text.PlainText; Layout.fillWidth:true; Accessible.role:Accessible.AlertMessage}
  GridLayout {
   columns: width < Kirigami.Units.gridUnit * 18 ? 2 : 3
   rowSpacing: Kirigami.Units.smallSpacing
   columnSpacing: Kirigami.Units.smallSpacing
   Layout.fillWidth:true
   Layout.fillHeight:true
   Repeater {
    model:root.view.cards
    Controls.Frame {
     id: card
     required property var modelData
     Layout.fillWidth:true
     Layout.fillHeight:true
     Layout.minimumWidth:Kirigami.Units.gridUnit*5
     Layout.minimumHeight:Kirigami.Units.gridUnit*5
     Accessible.name:modelData.title
     ColumnLayout {
      anchors.fill:parent
      Controls.Label {text:card.modelData.title; textFormat:Text.PlainText; wrapMode:Text.Wrap; Layout.fillWidth:true; font.bold:true}
      Controls.Label {text:card.modelData.available?card.modelData.values.join("\n"):"—"; textFormat:Text.PlainText; wrapMode:Text.Wrap; Layout.fillWidth:true; font.pixelSize:Kirigami.Units.gridUnit*1.2}
      Controls.Label {text:Mini.text("unavailable",root.view.language);visible:!card.modelData.available;textFormat:Text.PlainText;wrapMode:Text.Wrap;Layout.fillWidth:true}
     }
    }
   }
  }
 }
}
