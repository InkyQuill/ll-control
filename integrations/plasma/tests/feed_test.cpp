// SPDX-FileCopyrightText: 2026 Pavel Obruchnikov <me@inkyquill.net>
// SPDX-License-Identifier: GPL-3.0-or-later
#include "feed.h"
#include <QtTest>
#include <QTemporaryDir>
#include <QFile>
#include <QSignalSpy>
#include <QQmlEngine>
#include <QQmlComponent>
#include <csignal>
#include <cerrno>

class FeedTest:public QObject {
 Q_OBJECT
 QByteArray m_path;
 void script(const QString &directory,const QByteArray &body) {
  QFile file(directory+"/lconnectctl");QVERIFY(file.open(QIODevice::WriteOnly));
  file.write("#!/usr/bin/python3\nimport sys,time,os,json\nassert sys.argv[1:]==['mini','watch']\n"+body);file.close();QVERIFY(file.setPermissions(QFileDevice::ReadOwner|QFileDevice::WriteOwner|QFileDevice::ExeOwner));qputenv("PATH",directory.toUtf8());
 }
private Q_SLOTS:
 void init(){m_path=qgetenv("PATH");}
 void cleanup(){qputenv("PATH",m_path);}
 void fragmentedLinesUpdateAndDisableReapsOnlyTheOwnedHelper() {
  QTemporaryDir root;script(root.path(),"sys.stdout.write('{\"version\":1,');sys.stdout.flush();time.sleep(.02)\nsys.stdout.write('\"available\":true,\"data\":{\"sample\":1}}\\n{\"version\":1,\"available\":true,\"data\":{\"sample\":2}}\\n');sys.stdout.flush();time.sleep(20)\n");
  Feed feed;feed.setActive(true);QTRY_COMPARE_WITH_TIMEOUT(feed.frame().value("data").toMap().value("sample").toInt(),2,3000);const auto pid=feed.helperPid();QVERIFY(pid>0);QVERIFY(feed.elapsedMs()<1000);
  feed.setActive(false);QCOMPARE(feed.frame().value("available").toBool(),false);QCOMPARE(feed.helperPid(),0);QVERIFY(::kill(pid,0)==-1&&errno==ESRCH);
 }
 void malformedAndOversizedFramesDiscardOldData() {
  for(const QByteArray &bad:{QByteArray("sys.stdout.write('not JSON\\n')"),QByteArray("sys.stdout.write('x'*(1024*1024+1))")}) {
   QTemporaryDir root;script(root.path(),"print('{\"version\":1,\"available\":true,\"data\":{\"sample\":1}}',flush=True)\ntime.sleep(.1)\n"+bad+"\nsys.stdout.flush();time.sleep(20)\n");
   Feed feed;feed.setActive(true);QTRY_VERIFY_WITH_TIMEOUT(feed.frame().value("available").toBool(),3000);QTRY_VERIFY_WITH_TIMEOUT(!feed.frame().value("available").toBool(),3000);QVERIFY(!feed.frame().contains("data"));feed.setActive(false);QCOMPARE(feed.helperPid(),0);
  }
 }
 void silentHelperExpiresAndRestartRecovers() {
  QTemporaryDir root;script(root.path(),"p=os.path.join(os.path.dirname(__file__),'count')\nn=int(open(p).read())+1 if os.path.exists(p) else 1\nopen(p,'w').write(str(n))\nprint(json.dumps({'version':1,'available':True,'data':{'sample':n}}),flush=True)\nif n>1:time.sleep(20)\n");
  Feed feed;feed.setActive(true);QTRY_COMPARE_WITH_TIMEOUT(feed.frame().value("data").toMap().value("sample").toInt(),2,5000);QTRY_VERIFY_WITH_TIMEOUT(!feed.frame().value("available").toBool(),7000);QVERIFY(!feed.frame().contains("data"));feed.setActive(false);QCOMPARE(feed.helperPid(),0);
 }
 void missingHelperRemainsUnavailableAndCanBeDisabled() {
  QTemporaryDir root;qputenv("PATH",root.path().toUtf8());Feed feed;feed.setActive(true);QVERIFY(!feed.frame().value("available").toBool());feed.setActive(false);QTest::qWait(1100);QCOMPARE(feed.helperPid(),0);
 }
};
QTEST_MAIN(FeedTest)
#include "feed_test.moc"
