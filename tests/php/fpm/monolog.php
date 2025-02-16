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

use Monolog\Logger;
use Monolog\Handler\StreamHandler;

require_once dirname(__DIR__) . "/vendor/autoload.php";

class MyString {
    private $str;

    public function __construct($str) {
        $this->str = $str;
    }
    public function __toString() {
        return $this->str;
    }
}

$logger = new Logger('my_logger');

$logger->info('This is a INFO level log.');
$logger->warning('This is a WARNING level log.');
$logger->error(new MyString('This is a ERROR level log.'), [
    "foo" => 123, "bar" => false, "baz" => new MyString("test"),
]);

echo "ok";
