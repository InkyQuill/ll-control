// SPDX-FileCopyrightText: 2026 Pavel Obruchnikov <me@inkyquill.net>
// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <QObject>
#include <QVariantMap>
#include <QProcess>
#include <QTimer>
#include <QElapsedTimer>
#include <QtQml/qqmlregistration.h>

class Feed : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(bool active READ active WRITE setActive NOTIFY activeChanged)
    Q_PROPERTY(QString frameJson READ frameJson NOTIFY frameChanged)
    Q_PROPERTY(int elapsedMs READ elapsedMs NOTIFY elapsedChanged)
public:
    explicit Feed(QObject *parent=nullptr);
    ~Feed() override;
    bool active() const {return m_active;}
    void setActive(bool active);
    QVariantMap frame() const {return m_frame;}
    QString frameJson() const {return m_frameJson;}
    int elapsedMs() const;
    qint64 helperPid() const {return m_process.processId();}
Q_SIGNALS:
    void activeChanged();
    void frameChanged();
    void elapsedChanged();
private:
    void start();
    void stop();
    void read();
    void unavailable();
    void failed();
    bool m_active=false;
    QProcess m_process;
    QTimer m_retry;
    QTimer m_clock;
    QElapsedTimer m_received;
    QByteArray m_buffer;
    QVariantMap m_frame;
    QString m_frameJson=QStringLiteral("{\"version\":1,\"available\":false}");
};
