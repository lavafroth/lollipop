import QtQuick
import QtQuick.Layouts
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasmoid
import org.kde.plasma.workspace.dbus 1.0

import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.core as PlasmaCore
import org.kde.kirigami as Kirigami

PlasmoidItem {
    id: root
    preferredRepresentation: PlasmoidItem.FullRepresentation
    compactRepresentation: fullRepresentation
    
    fullRepresentation: Item {
        width: Kirigami.Units.gridUnit * 3
        height: Kirigami.Units.gridUnit
        implicitWidth: statusText.contentWidth + (Kirigami.Units.largeSpacing * 2)
        implicitHeight: Kirigami.Units.gridUnit

        // TODO: most of this is trial and error, some lines might be useless here
        Layout.minimumWidth: Kirigami.Units.gridUnit * 3
        Layout.minimumHeight: Kirigami.Units.gridUnit
        Layout.preferredWidth: implicitWidth
        Layout.preferredHeight: implicitHeight



        Text {
            id: statusText
            anchors.centerIn: parent
            text: ""
            color: "white"
        }

        SignalWatcher {
            busType: BusType.Session
            service: "xyz.lavafroth.Lollipop"
            path: "/Object"
            iface: "xyz.lavafroth.Lollipop"
            function dbusFileChanged(path) {
                statusText.text = path
            }
        }
    }

}

