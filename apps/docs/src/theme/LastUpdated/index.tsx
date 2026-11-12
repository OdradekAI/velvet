import React, {useEffect, useState, type ReactNode} from 'react';
import Translate from '@docusaurus/Translate';
import {ThemeClassNames} from '@docusaurus/theme-common';
import {useDateTimeFormat} from '@docusaurus/theme-common/internal';

type LastUpdatedProps = {
  readonly lastUpdatedAt?: number;
  readonly lastUpdatedBy?: string;
};

function useUserTimeZone(): string | undefined {
  const [timeZone, setTimeZone] = useState<string>();

  useEffect(() => {
    setTimeZone(Intl.DateTimeFormat().resolvedOptions().timeZone);
  }, []);

  return timeZone;
}

function LastUpdatedAtDate({lastUpdatedAt}: {lastUpdatedAt: number}): ReactNode {
  const atDate = new Date(lastUpdatedAt);
  const userTimeZone = useUserTimeZone();
  const dateTimeFormat = useDateTimeFormat({
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    timeZone: userTimeZone ?? 'UTC',
  });

  return (
    <Translate
      id="theme.lastUpdated.atDate"
      description="The words used to describe on which date a page has been last updated"
      values={{
        date: (
          <b>
            <time
              dateTime={atDate.toISOString()}
              itemProp="dateModified"
              suppressHydrationWarning
              title={userTimeZone ? `Timezone: ${userTimeZone}` : undefined}
            >
              {dateTimeFormat.format(atDate)}
            </time>
          </b>
        ),
      }}>
      {' on {date}'}
    </Translate>
  );
}

function LastUpdatedByUser({lastUpdatedBy}: {lastUpdatedBy: string}): ReactNode {
  return (
    <Translate
      id="theme.lastUpdated.byUser"
      description="The words used to describe by who the page has been last updated"
      values={{user: <b>{lastUpdatedBy}</b>}}>
      {' by {user}'}
    </Translate>
  );
}

export default function LastUpdated({lastUpdatedAt, lastUpdatedBy}: LastUpdatedProps): ReactNode {
  return (
    <span className={ThemeClassNames.common.lastUpdated}>
      <Translate
        id="theme.lastUpdated.lastUpdatedAtBy"
        description="The sentence used to display when a page has been last updated, and by who"
        values={{
          atDate: lastUpdatedAt ? <LastUpdatedAtDate lastUpdatedAt={lastUpdatedAt} /> : '',
          byUser: lastUpdatedBy ? <LastUpdatedByUser lastUpdatedBy={lastUpdatedBy} /> : '',
        }}>
        {'Last updated{atDate}{byUser}'}
      </Translate>
    </span>
  );
}
