import { Layout } from "~/components/layout/Layout.tsx";
import { Prose } from "~/components/ui/Prose";

export const Privacy = () => {
  return (
    <Layout>
      <div class="mt-6 mb-4 flex justify-around">
        <Prose class="max-w-[90ch]">
          <h1 id="vNAS Stats-privacy-policy">vNAS Stats Privacy Policy</h1>
          <h2 id="purpose">Purpose</h2>
          <p>
            The purpose of this policy is to describe how vNAS Stats collects, uses, and shares
            personal information.
          </p>
          <h2 id="applicability">Applicability</h2>
          <p>
            This policy applies to any current or former user of vNAS Stats who has authenticated
            with vNAS Stats using a VATSIM Certificate Identification (CID) number via VATSIM
            Connect.
          </p>
          <h2 id="policy">Policy</h2>
          <h3 id="1-registration">1. Registration</h3>
          <p>
            Use of vNAS Stats does not require registration with vNAS Stats services. Users must
            have registered with, and provide, a VATSIM CID to authenticate with vNAS Stats. VATSIM
            maintains its own Privacy Policy which can be found on its website.
          </p>
          <h3 id="2-information-collection-and-use">2. Information Collection and Use</h3>
          <p>
            vNAS Stats may collect information provided by VATSIM, including but not limited to,
            users’ CIDs and names, for users that authenticate with vNAS Stats via VATSIM Connect.
            vNAS Stats will not share, sell or rent this information to other parties.
          </p>
          <p>In compliance with GDPR, we process data under the following lawful bases:</p>
          <ul>
            <li>
              <strong>Consent</strong>: when authenticating with vNAS Stats via VATSIM Connect.
            </li>
            <li>
              <strong>Contractual necessity</strong>: to provide access to our services.
            </li>
            <li>
              <strong>Legitimate interest</strong>: for system integrity.
            </li>
            <li>
              <strong>Legal obligation</strong>: to comply with applicable requirements for VATSIM.
            </li>
          </ul>
          <h3 id="3-your-privacy-rights-gdpr-and-ccpa-">3. Your Privacy Rights (GDPR and CCPA)</h3>
          <p>
            Under GDPR and CCPA (California Residents), users of vNAS Stats have the following
            rights:
          </p>
          <ul>
            <li>
              A right to be informed about why we collect your data. This policy explains these
              reasons to you.
            </li>
            <li>
              A right to access your data and view the data collected on you. This data will be
              released to you on written request.
            </li>
            <li>A right to rectify data you believe to be inaccurate.</li>
            <li>The right to request your personal data be erased from the vNAS Stats systems.</li>
            <li>
              The right to object to our legitimate interests. While your objection is being
              considered, processing of your data may be frozen, and this may restrict your ability
              to authenticate via vNAS Stats.
            </li>
          </ul>
          <p>
            You can exercise any of these rights by contacting{" "}
            <a href="mailto: admin@vnas-stats.com">admin@vnas-stats.com</a>.
          </p>
          <h3 id="4-personal-statistical-information-and-logging">
            4. Personal Statistical Information and Logging
          </h3>
          <p>
            In the normal course of authenticating with vNAS Stats via VATSIM Connect, certain
            personal statistical information may be collected. This information includes, but is not
            limited to, name, CID and time of connection. This information will not be shared
            publicly or with 3rd parties, but statistical information may be aggregated and shared
            in an anonymized manner.
          </p>
          <h3 id="5-data-retention">5. Data Retention</h3>
          <p>
            We retain personal data only as long as necessary to fulfill the purposes outlined in
            this policy. Data related to user accounts, exams, logs, and support is retained for up
            to 24 months after inactivity unless required otherwise by law or upon user request.
          </p>
          <h3 id="6-international-data-transfers">6. International Data Transfers</h3>
          <p>
            Your data may be stored and processed in the United States, United Kingdom, or European
            Union. We apply appropriate safeguards, including Standard Contractual Clauses (SCCs),
            to ensure lawful international data transfers under GDPR.
          </p>
          <h3 id="7-children-s-privacy">7. Children’s Privacy</h3>
          <p>
            Our services are not intended for children under the age of 16. Access requires a VATSIM
            account, which is restricted to individuals aged 16 or older. We do not knowingly
            collect data from children under this age.
          </p>
          <p>
            If you believe we have inadvertently collected such data, please contact us immediately
            at <a href="mailto: admin@vnas-stats.com">admin@vnas-stats.com</a> to request deletion.
          </p>
          <h3 id="8-cookies">8. Cookies</h3>
          <p>
            vNAS Stats may use cookies on its site. A cookie is a piece of data stored on your hard
            drive which is used to track you through our site and for network security.
          </p>
          <h3 id="9-links-to-external-websites-and-services">
            9. Links to External Websites and Services
          </h3>
          <p>
            The vNAS Stats website contains links to third-party sites. vNAS Stats is not
            responsible for the privacy practices or policies of such other sites. vNAS Stats
            encourages its users to be aware when they leave its website and to read the privacy
            statements of each and every web site that collects personally identifiable information.
            This privacy statement applies solely to information collected by the vNAS Stats website
            or by authenticating via vNAS Stats.
          </p>
          <h3 id="10-changes-to-this-policy">10. Changes to This Policy</h3>
          <p>
            We may update this Privacy Policy periodically. Any significant changes will be
            communicated on our platform or by email where applicable. Continued use of our services
            after updates constitutes acceptance of the revised terms.
          </p>
        </Prose>
      </div>
    </Layout>
  );
};
