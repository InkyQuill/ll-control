import org.kde.plasma.configuration
import org.kde.plasma.plasmoid
import "../code/mini.mjs" as Mini
ConfigModel {
 ConfigCategory {name:Mini.text("configuration",Plasmoid.configuration?.languageCache==="ru-RU"?"ru-RU":"en-US"); icon:"utilities-system-monitor"; source:"configGeneral.qml"}
}
