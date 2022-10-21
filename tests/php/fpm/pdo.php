<?php

// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use Webmozart\Assert\Assert;

require_once dirname(__DIR__) . "/vendor/autoload.php";

{
    $pdo = new PDO("mysql:dbname=skywalking;host=127.0.0.1;port=3306", "root", "password");
    $result = $pdo->exec("SELECT 1");
    Assert::notFalse($result);
}

{
    $pdo = new PDO("mysql:dbname=skywalking;host=127.0.0.1:3306", "root", "password");
    $sth = $pdo->prepare("SELECT * FROM `mysql`.`user` WHERE `User` = :user", [PDO::ATTR_CURSOR => PDO::CURSOR_FWDONLY]);
    $sth->execute(['user' => 'root']);
    $rs = $sth->fetchAll();
    Assert::same(count($rs), 2);
}

{
    $pdo = new PDO("mysql:dbname=skywalking;host=127.0.0.1:3306;", "root", "password");
    $sth = $pdo->prepare("SELECT * FROM `mysql`.`user` WHERE `User` = :user", [PDO::ATTR_CURSOR => PDO::CURSOR_FWDONLY]);
    $sth->execute(['user' => 'anon']);
    $rs = $sth->fetchAll();
    Assert::same(count($rs), 0);
}

{
    Assert::throws(function () {
        $pdo = new PDO("mysql:dbname=skywalking;host=127.0.0.1;port=3306", "root", "password");
        $pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
        $sth = $pdo->prepare("SELECT * FROM not_exist");
        $sth->execute();
    }, PDOException::class);
}

echo "ok";
