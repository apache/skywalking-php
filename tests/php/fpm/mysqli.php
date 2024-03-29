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
    $mysqli = new mysqli("127.0.0.1", "root", "password", "skywalking", 3306);
    $result = $mysqli->query("SELECT 1");
    Assert::notFalse($result);
}

{
    $mysqli = new mysqli("127.0.0.1", "root", "password", "skywalking", 3306);
    $result = $mysqli->query("SELECT * FROM `mysql`.`user` WHERE `User` = 'root'");
    $rs = $result->fetch_all();
    Assert::same(count($rs), 2);
}

{
    $mysqli = mysqli_connect("127.0.0.1", "root", "password", "skywalking", 3306);
    $result = mysqli_query($mysqli, "SELECT * FROM `mysql`.`user` WHERE `User` = 'root'");
    $rs = $result->fetch_all();
    Assert::same(count($rs), 2);
}

{
    mysqli_report(MYSQLI_REPORT_OFF);
    $mysqli = mysqli_init();
    @mysqli_real_connect($mysqli, "127.0.0.1", "root", "password_incorrect", "skywalking", 3306);
}

echo "ok";
